//! Public downstream contract tests using real native budgets and selected
//! physical Jemalloc. This executable-only observer is not a production hook.
use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::ptr::NonNull;
use std::sync::{Arc, Barrier};
use tidb_tiforth_memory::tiforth_memory::{
    AdmissionV1, MemoryError, MetadataPlan, MetadataSlabError, RawAllocator, SiteId,
};
use tidb_tiforth_memory::*;

thread_local! {
    static PROBE: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}
struct JemallocObserver;
#[global_allocator]
static OBSERVER: JemallocObserver = JemallocObserver;

fn observe() {
    if PROBE.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
    }
}

// SAFETY: every operation forwards unchanged to the actual selected Jemalloc
// global allocator. Thread-local integer observation owns no heap storage.
unsafe impl GlobalAlloc for JemallocObserver {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        observe();
        unsafe { tikv_jemallocator::Jemalloc.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { tikv_jemallocator::Jemalloc.dealloc(ptr, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        observe();
        unsafe { tikv_jemallocator::Jemalloc.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        observe();
        unsafe { tikv_jemallocator::Jemalloc.realloc(ptr, layout, size) }
    }
}

fn probe<R>(f: impl FnOnce() -> R) -> (R, usize) {
    ALLOCS.with(|c| c.set(0));
    PROBE.with(|c| c.set(true));
    let result = f();
    PROBE.with(|c| c.set(false));
    (result, ALLOCS.with(Cell::get))
}

const SITE: SiteId = SiteId {
    version: 1,
    site: 1,
};
type Service = NativeService<2>;
type Root = NativeRoot<2, 8, 32>;

fn reserved(pool: &Arc<ResourcePool>, bytes: usize) -> Budget {
    let mut budget = pool.create_budget();
    budget.reserve(i64::try_from(bytes).unwrap()).unwrap();
    assert_eq!(budget.capacity(), bytes as i64);
    budget
}

fn initialize<const N: usize>(service: &'static NativeService<N>) -> Arc<ResourcePool> {
    let pool = ResourcePool::new_default("dedicated-tiforth", 1);
    pool.start(None, 1 << 30);
    let mut credit = reserved(&pool, NativeService::<N>::service_bytes());
    service.initialize(&mut credit).unwrap();
    assert_eq!(credit.capacity(), 0);
    pool
}

fn register(service: &Service, pool: &Arc<ResourcePool>, epoch: u64, payload: usize) -> usize {
    let request = Service::root_request::<2, 8, 32>(epoch, payload).unwrap();
    let mut credit = reserved(pool, request.total_bytes);
    service
        .reserve_query::<2, 8, 32>(epoch, payload, &mut credit)
        .unwrap();
    assert_eq!(credit.capacity(), 0);
    request.total_bytes
}

#[test]
fn native_credit_retained_until_last_physical_free() {
    static SERVICE: Service = Service::new();
    let pool = initialize(&SERVICE);
    let envelope = register(&SERVICE, &pool, 1, 32_768);
    let root: Root = SERVICE.try_root(1).unwrap();
    let query = root.open_query(32_768).unwrap();
    let mut payload = query
        .try_allocate(Layout::from_size_align(65, 64).unwrap(), SITE)
        .unwrap();
    payload.as_mut_slice()[0..5].copy_from_slice(b"bytes");
    let plan = MetadataPlan::new([Layout::new::<[u8; 5]>(), Layout::new::<u64>()]);
    let slab = query.prepare_metadata(plan, SITE).unwrap();
    let mut block = slab
        .try_allocate(slab.slot(0).unwrap(), plan.layouts()[0])
        .unwrap();
    block.as_mut_slice().copy_from_slice(b"hello");
    assert!(payload.as_slice()[5..].iter().all(|v| *v == 0));
    assert_eq!(SERVICE.allocator().snapshot().entries, 3);
    query.close_admission();
    assert!(matches!(
        slab.try_allocate(slab.slot(1).unwrap(), plan.layouts()[1]),
        Err(MetadataSlabError::Closed)
    ));
    SERVICE.request_close(1).unwrap();
    assert_eq!(SERVICE.flush(1), Err(BridgeError::PendingOwners));
    drop(slab);
    drop(query);
    drop(root);
    SERVICE.shutdown().unwrap();
    // This unchanged public invariant is the premature-native-refund negative
    // target. It runs before any access/free of surviving physical owners.
    assert_eq!(
        pool.allocated(),
        (Service::service_bytes() + envelope) as i64,
        "native envelope must remain charged while physical owners survive"
    );
    assert_eq!(SERVICE.snapshot().releases, 0);
    std::thread::spawn(move || {
        assert_eq!(&payload.as_slice()[0..5], b"bytes");
        drop(payload);
        assert_eq!(SERVICE.allocator().snapshot().frees, 1);
        assert_eq!(block.as_slice(), b"hello");
        drop(block);
    })
    .join()
    .unwrap();
    let physical = SERVICE.allocator().snapshot();
    assert_eq!(physical.frees, 3);
    assert_eq!(SERVICE.snapshot().releases, 1);
    assert_eq!(SERVICE.snapshot().frees_at_last_release, 3);
    assert_eq!(
        pool.allocated(),
        (Service::service_bytes() + envelope) as i64
    );
    SERVICE.flush(1).unwrap();
    assert_eq!(pool.allocated(), Service::service_bytes() as i64);
    assert_eq!(SERVICE.flush(1), Err(BridgeError::StaleEpoch));
    println!(
        "native trace: entries=3 free=3 release_after_free=3 flush=1 service_retained={}",
        pool.allocated()
    );
}

#[test]
fn capacity_denial_has_zero_backend_entries_and_no_root_refund() {
    static SERVICE: Service = Service::new();
    let pool = initialize(&SERVICE);
    assert!(SERVICE.try_root::<2, 8, 32>(1).is_err());
    assert_eq!(SERVICE.allocator().snapshot().entries, 0);
    let envelope = register(&SERVICE, &pool, 1, 64);
    let root: Root = SERVICE.try_root(1).unwrap();
    let query = root.open_query(64).unwrap();
    let before = SERVICE.allocator().snapshot();
    assert!(matches!(
        query.try_allocate(Layout::new::<[u8; 65]>(), SITE),
        Err(MemoryError::Denied)
    ));
    assert_eq!(SERVICE.allocator().snapshot(), before);
    let payload = query.try_allocate(Layout::new::<[u8; 64]>(), SITE).unwrap();
    assert!(query.try_allocate(Layout::new::<u8>(), SITE).is_err());
    assert_eq!(SERVICE.allocator().snapshot().entries, before.entries + 1);
    drop(query);
    drop(root);
    assert_eq!(
        pool.allocated(),
        (Service::service_bytes() + envelope) as i64
    );
    drop(payload);
    SERVICE.flush(1).unwrap();
    assert_eq!(pool.allocated(), Service::service_bytes() as i64);
}

#[test]
fn exact_padded_classes_match_real_usable_storage() {
    let allocator = NativeJemalloc::new();
    for (size, align) in [
        (1, 1),
        (9, 8),
        (65, 64),
        (513, 256),
        (4097, 4096),
        (17, 65_536),
        (100_003, 128),
    ] {
        let retained =
            NativeJemalloc::retained_layout(Layout::from_size_align(size, align).unwrap()).unwrap();
        let layout = retained.layout();
        let ptr = allocator.allocate(layout).unwrap();
        assert_eq!(ptr.as_ptr() as usize % align, 0);
        // SAFETY: ptr is this allocator's live block, never freed before query.
        assert_eq!(unsafe { allocator.usable_size(ptr) }, retained.charge());
        println!(
            "class size={size} align={align} padded={} usable={}",
            layout.size(),
            retained.charge()
        );
        // SAFETY: unique matching block/layout.
        unsafe { allocator.deallocate(ptr, layout) };
    }
    assert!(NativeJemalloc::retained_layout(Layout::new::<()>()).is_err());
    assert!(Service::root_request::<2, 8, 32>(1, usize::MAX).is_err());
    assert_eq!(
        allocator.snapshot(),
        AllocatorSnapshot {
            entries: 7,
            frees: 7
        }
    );
}

#[test]
fn rejects_poolless_negative_surplus_stopped_and_mismatched_native_budgets() {
    static SERVICE: Service = Service::new();
    let mut none = Budget::default();
    assert_eq!(
        SERVICE.initialize(&mut none),
        Err(BridgeError::InvalidBudget)
    );
    let pool = initialize(&SERVICE);
    assert_eq!(
        SERVICE.initialize(&mut none),
        Err(BridgeError::AlreadyInitialized)
    );
    let req = Service::root_request::<2, 8, 32>(1, 64).unwrap();
    let mut wrong = reserved(&pool, req.total_bytes + 1);
    assert_eq!(
        SERVICE.reserve_query::<2, 8, 32>(1, 64, &mut wrong),
        Err(BridgeError::InvalidBudget)
    );
    wrong.clear();
    let mut negative = pool.create_budget();
    negative.grow(-1).unwrap();
    assert_eq!(
        SERVICE.reserve_query::<2, 8, 32>(1, 64, &mut negative),
        Err(BridgeError::InvalidBudget)
    );
    let other = ResourcePool::new_default("other", 1);
    other.start(None, 1 << 20);
    let mut wrong_pool = reserved(&other, req.total_bytes);
    assert_eq!(
        SERVICE.reserve_query::<2, 8, 32>(1, 64, &mut wrong_pool),
        Err(BridgeError::PoolMismatch)
    );
    wrong_pool.clear();
    assert_eq!(SERVICE.allocator().snapshot().entries, 0);

    static STOPPED: Service = Service::new();
    let stopped = ResourcePool::new_default("stopped", 1);
    stopped.start(None, 1 << 20);
    let mut credit = reserved(&stopped, Service::service_bytes());
    stopped.stop();
    assert_eq!(
        STOPPED.initialize(&mut credit),
        Err(BridgeError::InconsistentBacking)
    );
}

#[test]
fn slots_epochs_cancel_reuse_and_shutdown_are_explicit() {
    static SERVICE: Service = Service::new();
    let pool = initialize(&SERVICE);
    register(&SERVICE, &pool, 1, 64);
    let mut duplicate = reserved(
        &pool,
        Service::root_request::<2, 8, 32>(1, 64)
            .unwrap()
            .total_bytes,
    );
    assert_eq!(
        SERVICE.reserve_query::<2, 8, 32>(1, 64, &mut duplicate),
        Err(BridgeError::StaleEpoch)
    );
    duplicate.clear();
    register(&SERVICE, &pool, 2, 64);
    let mut third = reserved(
        &pool,
        Service::root_request::<2, 8, 32>(3, 64)
            .unwrap()
            .total_bytes,
    );
    assert_eq!(
        SERVICE.reserve_query::<2, 8, 32>(3, 64, &mut third),
        Err(BridgeError::SlotsExhausted)
    );
    SERVICE.request_close(1).unwrap();
    SERVICE.flush(1).unwrap();
    SERVICE
        .reserve_query::<2, 8, 32>(3, 64, &mut third)
        .unwrap();
    let reused_root: Root = SERVICE.try_root(3).unwrap();
    let stale = Service::root_request::<2, 8, 32>(1, 64).unwrap();
    SERVICE.release_root(stale);
    assert_eq!(SERVICE.flush(3), Err(BridgeError::PendingOwners));
    assert!(SERVICE.try_root::<2, 8, 32>(1).is_err());
    SERVICE.shutdown().unwrap();
    assert!(SERVICE.try_root::<2, 8, 32>(2).is_err());
    SERVICE.flush(2).unwrap();
    assert_eq!(SERVICE.flush(3), Err(BridgeError::PendingOwners));
    drop(reused_root);
    SERVICE.flush(3).unwrap();
    assert_eq!(pool.allocated(), Service::service_bytes() as i64);
    let mut later = reserved(
        &pool,
        Service::root_request::<2, 8, 32>(4, 64)
            .unwrap()
            .total_bytes,
    );
    assert_eq!(
        SERVICE.reserve_query::<2, 8, 32>(4, 64, &mut later),
        Err(BridgeError::Closed)
    );
    later.clear();
}

#[test]
fn equal_envelope_close_duplicate_and_reused_slot_preserve_native_credit() {
    type Three = NativeService<3>;
    static SERVICE: Three = Three::new();
    let pool = initialize(&SERVICE);
    let register = |epoch| {
        let request = Three::root_request::<2, 8, 32>(epoch, 64).unwrap();
        let mut credit = reserved(&pool, request.total_bytes);
        SERVICE
            .reserve_query::<2, 8, 32>(epoch, 64, &mut credit)
            .unwrap();
        request
    };
    let first = register(1);
    let second = register(2);
    let target = register(3);
    assert_eq!(first.total_bytes, target.total_bytes);
    assert_eq!(second.total_bytes, target.total_bytes);
    let first_root: Root = SERVICE.try_root(1).unwrap();
    let second_root: Root = SERVICE.try_root(2).unwrap();
    let target_root: Root = SERVICE.try_root(3).unwrap();
    SERVICE.request_close(3).unwrap();
    assert_eq!(SERVICE.flush(3), Err(BridgeError::PendingOwners));
    std::thread::spawn(move || drop(target_root))
        .join()
        .unwrap();
    assert_eq!(SERVICE.snapshot().releases, 1);
    assert_eq!(SERVICE.snapshot().frees_at_last_release, 1);
    SERVICE.release_root(target); // Duplicate only after the real final free.
    assert_eq!(SERVICE.snapshot().releases, 1);
    SERVICE.flush(3).unwrap();
    assert_eq!(
        pool.allocated(),
        (Three::service_bytes() + 2 * target.total_bytes) as i64
    );
    let reused = register(4);
    let reused_root: Root = SERVICE.try_root(4).unwrap();
    SERVICE.release_root(target); // The old epoch must not release slot 3 again.
    assert_eq!(SERVICE.snapshot().releases, 1);
    assert_eq!(SERVICE.flush(4), Err(BridgeError::PendingOwners));
    assert_eq!(
        pool.allocated(),
        (Three::service_bytes() + 2 * target.total_bytes + reused.total_bytes) as i64
    );
    drop(reused_root);
    SERVICE.flush(4).unwrap();
    drop(first_root);
    drop(second_root);
    SERVICE.flush(1).unwrap();
    SERVICE.flush(2).unwrap();
    assert_eq!(SERVICE.snapshot().releases, 4);
    assert_eq!(SERVICE.allocator().snapshot().frees, 4);
    assert_eq!(pool.allocated(), Three::service_bytes() as i64);
}

#[test]
fn cold_callbacks_and_close_allocate_nothing_with_allocating_positive_control() {
    static SERVICE: Service = Service::new();
    let pool = initialize(&SERVICE);
    let envelope = register(&SERVICE, &pool, 1, 64);
    let request = Service::root_request::<2, 8, 32>(1, 64).unwrap();
    // First managed callback use of this static service; all native pool setup,
    // Budget/Arc ownership and size-class queries happened outside this probe.
    let (reserved, first) = probe(|| SERVICE.try_reserve_root(request));
    assert_eq!(reserved, Ok(()));
    assert_eq!(first, 0);
    let (closed, close_allocs) = probe(|| SERVICE.request_close(1));
    assert_eq!(closed, Ok(()));
    assert_eq!(close_allocs, 0);
    let (_, release_allocs) = probe(|| SERVICE.release_root(request));
    assert_eq!(release_allocs, 0);
    let (denied, denied_allocs) = probe(|| SERVICE.try_reserve_root(request));
    assert_eq!(denied, Err(MemoryError::Denied));
    assert_eq!(denied_allocs, 0);
    // Safety-preserving intentionally allocating control: the SAME selected
    // Jemalloc observer must detect a real allocation.
    let (positive, observed) = probe(|| {
        let mut bytes = Vec::with_capacity(37);
        bytes.push(1u8);
        std::hint::black_box(bytes)
    });
    assert!(observed > 0);
    drop(positive);
    assert_eq!(
        pool.allocated(),
        (Service::service_bytes() + envelope) as i64
    );
    assert_eq!(SERVICE.allocator().snapshot().entries, 0);
    SERVICE.flush(1).unwrap();
    println!("cold callback allocations reserve={first} close={close_allocs} release={release_allocs} denied={denied_allocs}; positive={observed}");
}

#[test]
fn simultaneous_last_capacity_and_cross_thread_release() {
    static SERVICE: Service = Service::new();
    let pool = initialize(&SERVICE);
    register(&SERVICE, &pool, 1, 64);
    let root: Root = SERVICE.try_root(1).unwrap();
    let query = Arc::new(root.open_query(64).unwrap());
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let q = Arc::clone(&query);
        let b = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            b.wait();
            q.try_allocate(Layout::new::<[u8; 64]>(), SITE)
        }));
    }
    barrier.wait();
    let results: Vec<_> = workers.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    drop(query);
    drop(root);
    assert_eq!(SERVICE.flush(1), Err(BridgeError::PendingOwners));
    std::thread::spawn(move || drop(results)).join().unwrap();
    SERVICE.flush(1).unwrap();
    assert_eq!(pool.allocated(), Service::service_bytes() as i64);
}

#[test]
fn duplicate_epoch_open_and_cancel_close_races_are_bounded() {
    static SERVICE: Service = Service::new();
    let pool = initialize(&SERVICE);
    for epoch in 1..=16 {
        register(&SERVICE, &pool, epoch, 64);
        let barrier = Arc::new(Barrier::new(3));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let b = Arc::clone(&barrier);
            threads.push(std::thread::spawn(move || {
                b.wait();
                SERVICE.try_root::<2, 8, 32>(epoch)
            }));
        }
        barrier.wait();
        let results: Vec<_> = threads.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        let close = std::thread::spawn(move || SERVICE.request_close(epoch));
        drop(results);
        assert_eq!(close.join().unwrap(), Ok(()));
        SERVICE.flush(epoch).unwrap();
        assert_eq!(pool.allocated(), Service::service_bytes() as i64);
    }
    // Open and unopened cancellation race against the same gate. Contention
    // is an explicit bounded result; it is retried only after both threads join.
    for epoch in 17..=32 {
        let envelope = register(&SERVICE, &pool, epoch, 64);
        let barrier = Arc::new(Barrier::new(3));
        let open_barrier = Arc::clone(&barrier);
        let open = std::thread::spawn(move || {
            open_barrier.wait();
            SERVICE.try_root::<2, 8, 32>(epoch)
        });
        let close_barrier = Arc::clone(&barrier);
        let close = std::thread::spawn(move || {
            close_barrier.wait();
            SERVICE.request_close(epoch)
        });
        barrier.wait();
        let root = open.join().unwrap();
        let closed = close.join().unwrap();
        assert!(matches!(closed, Ok(()) | Err(BridgeError::Contended)));
        SERVICE.request_close(epoch).unwrap();
        assert_eq!(
            pool.allocated(),
            (Service::service_bytes() + envelope) as i64
        );
        if root.is_ok() {
            assert_eq!(SERVICE.flush(epoch), Err(BridgeError::PendingOwners));
        }
        drop(root);
        SERVICE.flush(epoch).unwrap();
        assert_eq!(pool.allocated(), Service::service_bytes() as i64);
    }
}

#[test]
fn raw_fallible_invalid_request_retains_no_block() {
    let allocator = NativeJemalloc::new();
    assert_eq!(
        allocator.allocate(Layout::new::<()>()),
        Err(MemoryError::InvalidLayout)
    );
    assert_eq!(
        allocator.snapshot(),
        AllocatorSnapshot {
            entries: 0,
            frees: 0
        }
    );
    let _pointer_type: Option<NonNull<u8>> = None;
}
