//! Public-only native pool -> owned physical storage -> explicit safe refund.
use std::alloc::Layout;
use tidb_tiforth_memory::tiforth_memory::{MetadataPlan, SiteId};
use tidb_tiforth_memory::*;

static SERVICE: NativeService<1> = NativeService::new();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    type Root = NativeRoot<1, 4, 32>;
    let pool = ResourcePool::new_default("dedicated-tiforth-example", 1);
    pool.start(None, 1 << 20);
    let mut service_credit = pool.create_budget();
    service_credit.reserve(NativeService::<1>::service_bytes() as i64)?;
    SERVICE.initialize(&mut service_credit)?;
    let request = NativeService::<1>::root_request::<1, 4, 32>(1, 16_384)?;
    let mut envelope = pool.create_budget();
    envelope.reserve(request.total_bytes as i64)?;
    SERVICE.reserve_query::<1, 4, 32>(1, 16_384, &mut envelope)?;
    let root: Root = SERVICE.try_root(1)?;
    let query = root.open_query(16_384)?;
    let site = SiteId {
        version: 1,
        site: 1,
    };
    let mut payload = query.try_allocate(Layout::new::<[u8; 5]>(), site)?;
    payload.as_mut_slice().copy_from_slice(b"bytes");
    let plan = MetadataPlan::new([Layout::new::<[u8; 5]>()]);
    let slab = query
        .prepare_metadata(plan, site)
        .map_err(|e| format!("{e:?}"))?;
    let mut block = slab
        .try_allocate(
            slab.slot(0).map_err(|e| format!("{e:?}"))?,
            plan.layouts()[0],
        )
        .map_err(|e| format!("{e:?}"))?;
    block.as_mut_slice().copy_from_slice(b"hello");
    query.close_admission();
    SERVICE.request_close(1)?;
    drop(slab);
    drop(query);
    drop(root);
    SERVICE.shutdown()?;
    assert_eq!(SERVICE.flush(1), Err(BridgeError::PendingOwners));
    assert_eq!(
        pool.allocated(),
        (NativeService::<1>::service_bytes() + request.total_bytes) as i64
    );
    println!(
        "owners survive close: native={} backend={:?}",
        pool.allocated(),
        SERVICE.allocator().snapshot()
    );
    std::thread::spawn(move || {
        assert_eq!(payload.as_slice(), b"bytes");
        drop(payload);
        assert_eq!(block.as_slice(), b"hello");
        drop(block);
    })
    .join()
    .unwrap();
    assert_eq!(SERVICE.snapshot().frees_at_last_release, 3);
    SERVICE.flush(1)?;
    assert_eq!(pool.allocated(), NativeService::<1>::service_bytes() as i64);
    println!(
        "physical frees precede root release and control refund: {:?}; native service credit={}",
        SERVICE.allocator().snapshot(),
        pool.allocated()
    );
    Ok(())
}
