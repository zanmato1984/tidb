# Native pre-reserved memory bridge boundary

This additive bridge is for dedicated, already-reserved native
`tidb_memory::pool::Budget` credit and actual TiForth raw ownership. It does not
change Go, native Tracker/ResourcePool/arbitrator, the server allocator, Arrow,
SQL, scheduling, global allocation hooks, RSS or owner gate policy.

## Setup and persistent service credit

The host initializes native pools before managed execution, with positive valid
capacity and stable lifecycle. It reserves each Budget before handing ownership
to the bridge. Native setup may allocate, lock, format errors or call host
callbacks. These operations are not the AdmissionV1 callback contract.
The dedicated pool must not be stopped, restarted, reconfigured or manually
released while any bridge service/envelope is registered. Public native APIs do
not expose a revocation-proof sealed pool; general shared pools/arbitrators are
unsupported. The bridge checks pool identity, stopped state, exact reservation,
nonnegative consistent counters and actual native allocated backing at install.

NativeService is genuinely static finite storage, never leaked heap storage.
Its exact service_bytes includes its own allocator, controls and Budget slots
plus a separately admitted fixed TiForth slab-identity atomic. The service Budget
is retained for the static service lifetime, including after shutdown. Query
refund must leave that service credit in the actual native pool. Native Arc and
pool metadata were allocated in host setup; this bridge does not pretend that
existing host metadata became query-owned storage or an RSS proof.

## Credit and close ownership

A query envelope is exact payload capacity plus the complete root header's
selected Jemalloc class charge. Native Budget values live inside service slots.
Epochs strictly increase and are never reused. Atomic state transitions reject
duplicates, stale identities, exhausted slots, shutdown and contended admission.
Callbacks neither acquire native locks nor allocate. Final release publishes a
bounded atomic notification; control contention cannot discard it.

Close of service/root admission does not free or refund physical ownership.
TiForth OwnedRoot pins survive through PinnedAllocation and prepared slab blocks;
the actual last payload/slab/header free occurs before the root-release callback.
Only explicit flush on the host control plane clears native Budget once the
release notification is visible, or cancels an unopened envelope. A forgotten
owner keeps the entire envelope charged. There is no production reaper here.

Service close prevents new bridge roots, not transitive cancellation of already
opened TiForth queries. The host explicitly calls NativeQuery::close_admission
before dropping its query handles, as the public example does. Existing pins and
blocks remain usable; this service does not introduce an external query
arbitrator or silently close unrelated native trackers.

The record states are unopened, live, closing, release-notification-in-progress,
returned, cancelled, and free. Epoch and state share one atomic word. Unopened
close cancels before any backend entry; live close retains the envelope. Release
first acquire-loads and filters the unique matching epoch and live/closing state.
It claims that word with at most two strong compare-exchanges TOTAL per callback,
including a possible live-to-closing race, regardless of equal-sized envelopes
in other slots. It records physical-free observations, then publishes returned.
Explicit flush can clear only returned or unopened-cancelled credit, once.
Epoch values never repeat.

## Physical allocation

The bridge uses selected tikv Jemalloc 0.7 with profiling, matching host default,
without installing a library global allocator. Layouts are padded and their
same-version nallocx class is checked before fallible mallocx; representative
actual usable classes are compared using sallocx. The charge includes payload,
header/control padding and allocator size class, but not arena retention,
fragmentation, native allocator global metadata or process RSS.

The checked bridge wrappers are the supported route. Direct use of low-level
TiForth RawAllocator/RetainedLayout APIs remains the downstream caller's own
accounting responsibility. Direct low-level AdmissionV1 callback callers likewise
must send truthful accounting notifications: a release_root notification must
report that the final physical owner has already been freed. NativeRoot and
NativeQuery enforce their own checked ownership route; they do not validate
arbitrary manual callbacks.
No Arrow MetadataPermit, HostHardLimit, M01–M12,
production integrity/fail-stop decision, architecture acceptance or owner memory
gate is established by this slice.
