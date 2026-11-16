//! Fallible selected-native Jemalloc route; no global allocator is installed.
use std::alloc::Layout;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use tiforth_memory::{MemoryError, RawAllocator, RetainedLayout};
use tikv_jemalloc_sys as ffi;

/// Real backend observations, independent of native pool quota.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocatorSnapshot {
    pub entries: usize,
    pub frees: usize,
}

/// Fixed static service storage, included in NativeService::service_bytes().
pub struct NativeJemalloc {
    entries: AtomicUsize,
    frees: AtomicUsize,
}

impl NativeJemalloc {
    pub const fn new() -> Self {
        Self {
            entries: AtomicUsize::new(0),
            frees: AtomicUsize::new(0),
        }
    }

    /// Padded logical extent plus exact same-version usable size-class charge.
    /// nallocx is a size-class query, not a physical allocation.
    pub fn retained_layout(layout: Layout) -> Result<RetainedLayout, MemoryError> {
        let padded = RetainedLayout::new(layout.size(), layout.align(), 0)?;
        let layout = padded.layout();
        if layout.size() == 0 {
            return Err(MemoryError::InvalidLayout);
        }
        // SAFETY: valid nonzero Layout and power-of-two alignment. nallocx
        // returns zero for an unsupported class; it allocates no payload.
        let class = unsafe { ffi::nallocx(layout.size(), ffi::MALLOCX_ALIGN(layout.align())) };
        let overhead = class
            .checked_sub(layout.size())
            .filter(|_| class <= isize::MAX as usize)
            .ok_or(MemoryError::InvalidLayout)?;
        RetainedLayout::new(layout.size(), layout.align(), overhead)
    }

    pub fn snapshot(&self) -> AllocatorSnapshot {
        AllocatorSnapshot {
            entries: self.entries.load(Ordering::Acquire),
            frees: self.frees.load(Ordering::Acquire),
        }
    }

    /// Observe a live block obtained from this exact allocator.
    ///
    /// # Safety
    /// ptr must be a still-live pointer returned by this allocator.
    pub unsafe fn usable_size(&self, ptr: NonNull<u8>) -> usize {
        // SAFETY: caller supplies an actual live selected-Jemalloc block.
        unsafe { ffi::sallocx(ptr.as_ptr().cast(), 0) }
    }
}

impl Default for NativeJemalloc {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: mallocx returns unique aligned writable storage or null without owning
// a block. Matching dallocx is thread-safe and does not panic/unwind. There is
// no Rust allocation, callback, lock or formatting in this wrapper.
unsafe impl RawAllocator for NativeJemalloc {
    fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, MemoryError> {
        if layout.size() == 0 {
            return Err(MemoryError::InvalidLayout);
        }
        self.entries.fetch_add(1, Ordering::Relaxed);
        // SAFETY: Layout guarantees valid power-of-two alignment and size.
        NonNull::new(unsafe {
            ffi::mallocx(layout.size(), ffi::MALLOCX_ALIGN(layout.align())).cast::<u8>()
        })
        .ok_or(MemoryError::AllocationFailure)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, _layout: Layout) {
        // SAFETY: RawAllocator caller guarantees exact live allocation ownership.
        unsafe { ffi::dallocx(ptr.as_ptr().cast(), 0) };
        // This event is published only AFTER the actual allocator free.
        self.frees.fetch_add(1, Ordering::Release);
    }
}
