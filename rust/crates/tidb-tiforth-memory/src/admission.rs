//! Native Budget ownership is control-plane only; callbacks use fixed atomics.
use crate::{BridgeError, NativeJemalloc};
use std::cell::UnsafeCell;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tidb_memory::Budget;
use tiforth_memory::{AdmissionV1, MemoryError, RootRequest};

const FREE: u64 = 0;
const READY: u64 = 1;
const LIVE: u64 = 2;
const CLOSING: u64 = 3;
const CANCELLED: u64 = 4;
const RETURNED: u64 = 5;
const RETURNING: u64 = 6;
const MASK: u64 = 7;
const MAX_EPOCH: u64 = u64::MAX >> 3;

const fn token(epoch: u64, state: u64) -> u64 {
    (epoch << 3) | state
}

struct Slot {
    token: AtomicU64,
    payload: AtomicUsize,
    header: AtomicUsize,
    bytes: AtomicUsize,
    budget: UnsafeCell<Option<Budget>>,
}

impl Slot {
    const fn new() -> Self {
        Self {
            token: AtomicU64::new(FREE),
            payload: AtomicUsize::new(0),
            header: AtomicUsize::new(0),
            bytes: AtomicUsize::new(0),
            budget: UnsafeCell::new(None),
        }
    }

    fn matches(&self, request: RootRequest) -> bool {
        self.payload.load(Ordering::Relaxed) == request.payload_limit
            && self.header.load(Ordering::Relaxed) == request.bookkeeping_bytes
            && self.bytes.load(Ordering::Relaxed) == request.total_bytes
    }
}

/// Genuinely static, finite service. It never refunds its persistent service
/// credit, even on shutdown. Query envelopes require explicit control flush.
/// The host must preserve the dedicated pool's lifecycle while this exists.
pub struct NativeService<const N: usize> {
    gate: AtomicBool,
    initialized: AtomicBool,
    closed: AtomicBool,
    last_epoch: AtomicU64,
    service_budget: UnsafeCell<Option<Budget>>,
    slots: [Slot; N],
    pub(crate) allocator: NativeJemalloc,
    releases: AtomicUsize,
    frees_at_release: AtomicUsize,
    flushes: AtomicUsize,
}

// SAFETY: Budget cells are accessed only while owning gate; AdmissionV1 never
// reads them. Slots use atomic publication. Release touches only atomic tokens.
unsafe impl<const N: usize> Sync for NativeService<N> {}

struct Gate<'a>(&'a AtomicBool);
impl Drop for Gate<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceSnapshot {
    pub registered: usize,
    pub releases: usize,
    pub frees_at_last_release: usize,
    pub flushes: usize,
    pub closed: bool,
}

impl<const N: usize> NativeService<N> {
    pub const fn new() -> Self {
        Self {
            gate: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            last_epoch: AtomicU64::new(0),
            service_budget: UnsafeCell::new(None),
            slots: [const { Slot::new() }; N],
            allocator: NativeJemalloc::new(),
            releases: AtomicUsize::new(0),
            frees_at_release: AtomicUsize::new(0),
            flushes: AtomicUsize::new(0),
        }
    }

    /// Exact bridge static bytes plus the dependency's fixed slab identity
    /// atomic. Charging it per service conservatively duplicates that shared
    /// atomic when a process installs more than one service.
    pub const fn service_bytes() -> usize {
        size_of::<Self>() + size_of::<AtomicUsize>()
    }

    fn lock(&self) -> Result<Gate<'_>, BridgeError> {
        self.gate
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map(|_| Gate(&self.gate))
            .map_err(|_| BridgeError::Contended)
    }

    fn validate_budget(budget: &Budget, expected: usize) -> Result<(), BridgeError> {
        let expected = i64::try_from(expected).map_err(|_| BridgeError::InvalidBudget)?;
        let pool = budget.pool().ok_or(BridgeError::InvalidBudget)?;
        if expected <= 0
            || budget.capacity() != expected
            || budget.explicit_reserved() != expected
            || budget.used() != 0
        {
            return Err(BridgeError::InvalidBudget);
        }
        if pool.is_stopped() || pool.allocated() < expected || pool.limit() < expected {
            return Err(BridgeError::InconsistentBacking);
        }
        Ok(())
    }

    /// CONTROL ONLY: consumes the already-reserved exact service Budget on
    /// success; leaves the caller's Budget unchanged on error. May lock native
    /// pools. Service credit remains installed for this static service's life.
    pub fn initialize(&'static self, budget: &mut Budget) -> Result<(), BridgeError> {
        let _gate = self.lock()?;
        if self.initialized.load(Ordering::Relaxed) {
            return Err(BridgeError::AlreadyInitialized);
        }
        if self.closed.load(Ordering::Relaxed) {
            return Err(BridgeError::Closed);
        }
        Self::validate_budget(budget, Self::service_bytes())?;
        // SAFETY: gate exclusively owns all native Budget cells.
        unsafe { *self.service_budget.get() = Some(std::mem::take(budget)) };
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }

    pub(crate) fn register(
        &self,
        request: RootRequest,
        budget: &mut Budget,
    ) -> Result<(), BridgeError> {
        let _gate = self.lock()?;
        if !self.initialized.load(Ordering::Acquire) {
            return Err(BridgeError::Uninitialized);
        }
        if self.closed.load(Ordering::Relaxed) {
            return Err(BridgeError::Closed);
        }
        if request.root_epoch == 0 || request.root_epoch > MAX_EPOCH {
            return Err(BridgeError::EpochExhausted);
        }
        if request.root_epoch <= self.last_epoch.load(Ordering::Relaxed) {
            return Err(BridgeError::StaleEpoch);
        }
        Self::validate_budget(budget, request.total_bytes)?;
        // SAFETY: gate exclusively owns cell access.
        let service = unsafe { &*self.service_budget.get() }
            .as_ref()
            .ok_or(BridgeError::Uninitialized)?;
        let pool = service.pool().ok_or(BridgeError::InvalidBudget)?;
        if !Arc::ptr_eq(pool, budget.pool().ok_or(BridgeError::InvalidBudget)?) {
            return Err(BridgeError::PoolMismatch);
        }
        let mut backed = service.capacity();
        let mut free = None;
        for slot in &self.slots {
            // SAFETY: gate exclusively owns native Budget cell access.
            if let Some(installed) = unsafe { &*slot.budget.get() } {
                backed = backed
                    .checked_add(installed.capacity())
                    .ok_or(BridgeError::InvalidBudget)?;
            } else if free.is_none() {
                free = Some(slot);
            }
        }
        backed = backed
            .checked_add(budget.capacity())
            .ok_or(BridgeError::InvalidBudget)?;
        if pool.allocated() < backed {
            return Err(BridgeError::InconsistentBacking);
        }
        let slot = free.ok_or(BridgeError::SlotsExhausted)?;
        // SAFETY: gate exclusively owns cell, currently empty.
        unsafe { *slot.budget.get() = Some(std::mem::take(budget)) };
        slot.payload.store(request.payload_limit, Ordering::Relaxed);
        slot.header
            .store(request.bookkeeping_bytes, Ordering::Relaxed);
        slot.bytes.store(request.total_bytes, Ordering::Relaxed);
        slot.token
            .store(token(request.root_epoch, READY), Ordering::Release);
        self.last_epoch.store(request.root_epoch, Ordering::Relaxed);
        Ok(())
    }

    pub(crate) fn request(&self, epoch: u64) -> Result<RootRequest, BridgeError> {
        let _gate = self.lock()?;
        if epoch == 0 || epoch > MAX_EPOCH {
            return Err(BridgeError::StaleEpoch);
        }
        for slot in &self.slots {
            if slot.token.load(Ordering::Acquire) == token(epoch, READY) {
                return Ok(RootRequest {
                    root_epoch: epoch,
                    payload_limit: slot.payload.load(Ordering::Relaxed),
                    bookkeeping_bytes: slot.header.load(Ordering::Relaxed),
                    total_bytes: slot.bytes.load(Ordering::Relaxed),
                });
            }
        }
        Err(BridgeError::StaleEpoch)
    }

    fn close_slot(slot: &Slot, epoch: u64) -> Result<(), BridgeError> {
        let state = slot.token.load(Ordering::Acquire);
        if state >> 3 != epoch || state == FREE {
            return Err(BridgeError::StaleEpoch);
        }
        match state & MASK {
            READY => {
                slot.token.store(token(epoch, CANCELLED), Ordering::Release);
            }
            LIVE => {
                // Only release may race this gate-held operation.
                let _ = slot.token.compare_exchange(
                    state,
                    token(epoch, CLOSING),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
            }
            _ => {}
        }
        Ok(())
    }

    /// Nonallocating bounded bookkeeping only. No native refund occurs here.
    pub fn request_close(&self, epoch: u64) -> Result<(), BridgeError> {
        let _gate = self.lock()?;
        if epoch == 0 || epoch > MAX_EPOCH {
            return Err(BridgeError::StaleEpoch);
        }
        for slot in &self.slots {
            let state = slot.token.load(Ordering::Acquire);
            if state != FREE && state >> 3 == epoch {
                return Self::close_slot(slot, epoch);
            }
        }
        Err(BridgeError::StaleEpoch)
    }

    /// Stops new roots, cancels unopened envelopes, retains live owners and all
    /// service storage credit. Call flush explicitly on the host control plane.
    pub fn shutdown(&self) -> Result<(), BridgeError> {
        let _gate = self.lock()?;
        self.closed.store(true, Ordering::Release);
        for slot in &self.slots {
            let state = slot.token.load(Ordering::Acquire);
            if state != FREE {
                Self::close_slot(slot, state >> 3)?;
            }
        }
        Ok(())
    }

    /// CONTROL ONLY: native clear can block/lock. It is never called by
    /// AdmissionV1 or allocator/free callbacks. A pending live owner is an error.
    pub fn flush(&self, epoch: u64) -> Result<(), BridgeError> {
        let _gate = self.lock()?;
        if epoch == 0 || epoch > MAX_EPOCH {
            return Err(BridgeError::StaleEpoch);
        }
        for slot in &self.slots {
            let state = slot.token.load(Ordering::Acquire);
            if state == FREE || state >> 3 != epoch {
                continue;
            }
            if !matches!(state & MASK, RETURNED | CANCELLED) {
                return Err(BridgeError::PendingOwners);
            }
            // SAFETY: gate owns all Budget access; returned proves physical
            // header free, cancelled proves no allocator entry was admitted.
            if let Some(mut budget) = unsafe { &mut *slot.budget.get() }.take() {
                budget.clear();
            }
            slot.token.store(FREE, Ordering::Release);
            self.flushes.fetch_add(1, Ordering::Release);
            return Ok(());
        }
        Err(BridgeError::StaleEpoch)
    }

    pub fn allocator(&self) -> &NativeJemalloc {
        &self.allocator
    }

    pub fn snapshot(&self) -> ServiceSnapshot {
        ServiceSnapshot {
            registered: self
                .slots
                .iter()
                .filter(|s| s.token.load(Ordering::Acquire) != FREE)
                .count(),
            releases: self.releases.load(Ordering::Acquire),
            frees_at_last_release: self.frees_at_release.load(Ordering::Acquire),
            flushes: self.flushes.load(Ordering::Acquire),
            closed: self.closed.load(Ordering::Acquire),
        }
    }
}

impl<const N: usize> Default for NativeService<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> AdmissionV1 for NativeService<N> {
    fn try_reserve_root(&self, request: RootRequest) -> Result<(), MemoryError> {
        let _gate = self.lock().map_err(|_| MemoryError::Contended)?;
        if !self.initialized.load(Ordering::Acquire) || self.closed.load(Ordering::Acquire) {
            return Err(MemoryError::HostClosing);
        }
        if request.root_epoch == 0
            || request.root_epoch > MAX_EPOCH
            || request.payload_limit.checked_add(request.bookkeeping_bytes)
                != Some(request.total_bytes)
        {
            return Err(MemoryError::InvalidGrant);
        }
        for slot in &self.slots {
            if slot.token.load(Ordering::Acquire) == token(request.root_epoch, READY)
                && slot.matches(request)
            {
                slot.token
                    .store(token(request.root_epoch, LIVE), Ordering::Release);
                return Ok(());
            }
        }
        Err(MemoryError::Denied)
    }

    fn release_root(&self, request: RootRequest) {
        // Bounded scan and at most TWO strong CAS operations. Never takes gate
        // or native locks. Epoch is part of CAS, so late duplicate callbacks
        // cannot mark a reused slot returned. Close is the only possible
        // intervening state transition before the first valid release.
        if request.root_epoch == 0 || request.root_epoch > MAX_EPOCH {
            return;
        }
        for slot in &self.slots {
            if !slot.matches(request) {
                continue;
            }
            let first = slot.token.compare_exchange(
                token(request.root_epoch, LIVE),
                token(request.root_epoch, RETURNING),
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            let released = match first {
                Ok(_) => true,
                Err(actual) if actual == token(request.root_epoch, CLOSING) => slot
                    .token
                    .compare_exchange(
                        actual,
                        token(request.root_epoch, RETURNING),
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok(),
                _ => false,
            };
            if released {
                self.frees_at_release
                    .store(self.allocator.snapshot().frees, Ordering::Release);
                self.releases.fetch_add(1, Ordering::Release);
                slot.token
                    .store(token(request.root_epoch, RETURNED), Ordering::Release);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contention_is_bounded_and_release_notification_is_not_lost() {
        static SERVICE: NativeService<1> = NativeService::new();
        let pool = tidb_memory::ResourcePool::new_default("contention", 1);
        pool.start(None, 1 << 20);
        let mut service_credit = pool.create_budget();
        service_credit
            .reserve(NativeService::<1>::service_bytes() as i64)
            .unwrap();
        SERVICE.initialize(&mut service_credit).unwrap();
        let request = NativeService::<1>::root_request::<1, 1, 8>(1, 64).unwrap();
        let mut credit = pool.create_budget();
        credit.reserve(request.total_bytes as i64).unwrap();
        SERVICE
            .reserve_query::<1, 1, 8>(1, 64, &mut credit)
            .unwrap();
        let root = SERVICE.try_root::<1, 1, 8>(1).unwrap();
        let held = SERVICE.lock().unwrap();
        assert_eq!(
            SERVICE.try_reserve_root(request),
            Err(MemoryError::Contended)
        );
        drop(root); // Actual header free and release occur under gate contention.
        assert_eq!(
            SERVICE.slots[0].token.load(Ordering::Acquire),
            token(1, RETURNED)
        );
        SERVICE.release_root(request);
        assert_eq!(SERVICE.snapshot().releases, 1);
        assert_eq!(SERVICE.snapshot().frees_at_last_release, 1);
        assert_eq!(
            pool.allocated(),
            (NativeService::<1>::service_bytes() + request.total_bytes) as i64
        );
        drop(held);
        SERVICE.flush(1).unwrap();
        assert_eq!(pool.allocated(), NativeService::<1>::service_bytes() as i64);
    }
}
