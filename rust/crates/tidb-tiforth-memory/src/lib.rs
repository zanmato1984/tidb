//! Bounded native pre-reserved credit bridge, not Arrow/global/RSS enforcement.
#![deny(unsafe_op_in_unsafe_fn)]

mod admission;
mod allocator;
mod error;

pub use admission::{NativeService, ServiceSnapshot};
pub use allocator::{AllocatorSnapshot, NativeJemalloc};
pub use error::BridgeError;
pub use tidb_memory::{Budget, ResourcePool};
pub use tiforth_memory;

use std::alloc::Layout;
use tiforth_memory::{
    AuditSnapshot, Bookkeeping, MemoryError, MetadataPlan, MetadataSlabError, OwnedQuery,
    OwnedRoot, PinnedAllocation, PreparedMetadataSlab, RootRequest, SiteId,
};

/// Supported checked route to the actual OwnedRoot; no fabricated host counter.
pub struct NativeRoot<const Q: usize, const G: usize, const E: usize = 128> {
    inner: OwnedRoot<NativeJemalloc, Q, G, E>,
}

pub struct NativeQuery<const Q: usize, const G: usize, const E: usize = 128> {
    inner: OwnedQuery<NativeJemalloc, Q, G, E>,
}

impl<const N: usize> NativeService<N> {
    pub fn root_request<const Q: usize, const G: usize, const E: usize>(
        epoch: u64,
        payload: usize,
    ) -> Result<RootRequest, BridgeError> {
        let base = OwnedRoot::<NativeJemalloc, Q, G, E>::storage_layout(0)?;
        let header = NativeJemalloc::retained_layout(base.layout())?.charge();
        let total = payload
            .checked_add(header)
            .ok_or(BridgeError::InvalidLayout)?;
        if total > i64::MAX as usize || payload == 0 {
            return Err(BridgeError::InvalidLayout);
        }
        Ok(RootRequest {
            root_epoch: epoch,
            payload_limit: payload,
            bookkeeping_bytes: header,
            total_bytes: total,
        })
    }

    /// CONTROL ONLY: install exact already reserved credit; never allocates a
    /// root. On error budget ownership stays with the caller.
    pub fn reserve_query<const Q: usize, const G: usize, const E: usize>(
        &self,
        epoch: u64,
        payload: usize,
        budget: &mut Budget,
    ) -> Result<(), BridgeError> {
        self.register(Self::root_request::<Q, G, E>(epoch, payload)?, budget)
    }

    /// Construct actual physical root only after matching native credit exists.
    pub fn try_root<const Q: usize, const G: usize, const E: usize>(
        &'static self,
        epoch: u64,
    ) -> Result<NativeRoot<Q, G, E>, BridgeError> {
        let request = self.request(epoch)?;
        let required = Self::root_request::<Q, G, E>(epoch, request.payload_limit)?;
        if required != request {
            return Err(BridgeError::InvalidLayout);
        }
        let base = OwnedRoot::<NativeJemalloc, Q, G, E>::storage_layout(0)?;
        let overhead = request.bookkeeping_bytes - base.layout().size();
        Ok(NativeRoot {
            inner: OwnedRoot::try_new(
                self,
                &self.allocator,
                epoch,
                request.payload_limit,
                overhead,
                Bookkeeping::new(),
            )?,
        })
    }
}

impl<const Q: usize, const G: usize, const E: usize> NativeRoot<Q, G, E> {
    pub fn try_clone(&self) -> Result<Self, MemoryError> {
        Ok(Self {
            inner: self.inner.try_clone()?,
        })
    }

    pub fn open_query(&self, limit: usize) -> Result<NativeQuery<Q, G, E>, MemoryError> {
        Ok(NativeQuery {
            inner: self.inner.open_query(limit)?,
        })
    }

    pub fn audit_snapshot(&self) -> AuditSnapshot<E> {
        self.inner.budget().audit_snapshot()
    }
}

impl<const Q: usize, const G: usize, const E: usize> NativeQuery<Q, G, E> {
    pub fn close_admission(&self) {
        self.inner.close_admission();
    }

    pub fn audit_snapshot(&self) -> AuditSnapshot<E> {
        self.inner.audit_snapshot()
    }

    pub fn try_allocate(
        &self,
        layout: Layout,
        site: SiteId,
    ) -> Result<PinnedAllocation<NativeJemalloc, Q, G, E>, MemoryError> {
        self.inner
            .try_allocate(NativeJemalloc::retained_layout(layout)?, site)
    }

    pub fn prepare_metadata<const S: usize>(
        &self,
        plan: MetadataPlan<S>,
        site: SiteId,
    ) -> Result<PreparedMetadataSlab<NativeJemalloc, Q, G, E, S>, MetadataSlabError> {
        let base = PreparedMetadataSlab::<NativeJemalloc, Q, G, E, S>::storage_layout(&plan, 0)?;
        let charged = NativeJemalloc::retained_layout(base.layout())?;
        PreparedMetadataSlab::try_prepare(
            &self.inner,
            plan,
            charged.charge() - base.layout().size(),
            site,
        )
    }
}
