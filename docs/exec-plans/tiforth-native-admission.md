# Bridge pre-reserved native credit to physically owned TiForth storage

This ExecPlan is a living document maintained according to repository-root
`PLANS.md`. Keep Progress, Surprises & Discoveries, Decision Log, and Outcomes &
Retrospective current. The tracked prerequisite is TiForth issue #7, related to
memory issue #1. Neither this plan nor its tests approves an owner gate.

## Purpose / Big Picture


A Rust host can hand already reserved native ResourcePool Budget objects to a
finite static service, create an actually allocated TiForth root, and retain its
complete native quota until the last payload, prepared metadata slab, and root
header have physically been freed. Native refund is an explicit control-plane
operation, never an allocator callback. Run the public example to observe the
native pool retaining query plus service credit, then service credit alone.
This is not an Arrow route, SQL integration, global hook, or process RSS limit.

## Progress


- [x] (2026-11-16, R1 repair) Read the immutable repair brief and published
  required finding; checked the source-bound worker lease and project storage.
- [x] (2026-11-16, R1 repair) Record the fixed total two-CAS requirement before
  changing code. The original bbcfbcd candidate remains review-failed.
- [x] (2026-11-16, R1 repair) Repair epoch/state filtering and retain the bounded
  live-to-closing fallback and once-only returned publication.
- [x] (2026-11-16, R1 repair) Demonstrate unchanged old-red/current-green regression for
  three equal envelopes, including a closing target, and check late duplicates.
- [x] (2026-11-16, R1 repair) Run locked crate checks, strict Clippy, example and default Jemalloc server
  build; retain raw proof and stage exact products for independent review.

The earlier implementation checklist below describes the original candidate,
not acceptance of the R1 correction:

- [x] (2026-11-16) Read host/project instructions and immutable dispatch; checked
  project storage, archive identity and unexpired lease.
- [x] (2026-11-16) Write this plan and the native boundary document before code.
- [x] (2026-11-16) Implement the additive crate and real dependency lock graph.
- [x] (2026-11-16) Exercise native accounting, actual Jemalloc classes, ownership, races, and
  callback cold-first-use allocation controls.
- [x] (2026-11-16) Demonstrate safe premature-refund negative and unchanged current positive.
- [x] (2026-11-16) Run strict new-crate checks, original Chunk suite, and default server build.
- [x] (2026-11-16) Hash outputs, retain evidence, release Cargo and stage byte-identical files.

## Surprises & Discoveries


Published R1 (P2) found that the original release scan filtered envelope sizes
but not epoch before read-modify-write. Three equal envelopes at epochs 1/2/3
consume three strong compare-exchanges (CAS) for the last live envelope, or four
when it is closing. This violates the fixed total two-CAS bound; it is not an
observed wrong-refund or memory-safety failure. The required finding is
https://github.com/zanmato1984/tiforth/issues/7#issuecomment-6920694343.

The native Budget has no automatic refund on Drop. Its reserve and clear methods
can lock and invoke host policy; the bridge must own it and clear only in explicit
control operations. ResourcePool remains publicly mutable through Arc, so a
dedicated pool whose lifecycle the host keeps stable is a required precondition,
not an invented general arbitrator guarantee.

The first compile caught the native public export being `tidb_memory::Budget`,
not the private module path. Corrected only the new crate. The initial native
Chunk suite hit WouldBlock(35) in unchanged spill-directory initialization;
an identical second invocation passed all 272 tests with two test threads.
Source inspection suggests the existing first-use path-existence/nonblocking
file-lock race, not a bridge behavior change. The failure is retained.

Cargo reused a negative binary when switching back to older-mtime positive
source in the shared target. Cleaning only the new crate's generated package
artifacts forced an actual rebuild. The final negative uses the exact final
public test bytes, exits 101 at the intended retention assertion (248 native
bytes versus 36600 required), and the rebuilt current full suite passes.

## Decision Log


Decision: retain at most TWO strong CAS operations TOTAL per release callback,
not per slot. Acquire-load and filter the unique matching epoch and eligible
live/closing state before CAS; return after its single bounded claim sequence.
A live-to-closing race may use the second CAS. Test-only nonallocating counters
measure the actual production claim operations while real roots are freed.
Rationale: repair R1 without changing acceptance or replacing the production path.
Date/Author: 2026-11-16, assigned fresh R1 repair builder.

Decision: use real static service storage, separate persistent service Budget,
finite atomic epoch records and owned per-root Budget values. Rationale: static
TiForth services must not be fabricated with leaks, and release must never lose a
refund notification under contention. Date/Author: 2026-11-16, assigned builder.

Decision: use the host-selected tikv Jemalloc 0.7 profiling identity and direct
fallible mallocx/dallocx. Derive padded class charge using same-version nallocx.
Rationale: global allocator installation is unnecessary and aborting allocation
does not test fallibility. Arena fragmentation and RSS are explicitly outside
class charge. Date/Author: 2026-11-16, assigned builder.

Decision: pack non-reused epoch and lifecycle state into one AtomicU64 and use
at most two strong compare-exchanges for release, followed by final publication.
Rationale: late callbacks cannot mark reused slots returned, gate contention
cannot discard release, and native control flush sees completed notification.
Date/Author: 2026-11-16, assigned builder.

## Outcomes & Retrospective


R1 builder validation: the source-bound old release algorithm compiled with the
unchanged regression and actual-CAS counting helper, then failed with exit 101:
the third equal live envelope used 3 CAS and the closing envelope used 4.
The repaired production path passed both unchanged tests with 1 CAS each.
Truthful duplicates after final free and callbacks from an old epoch after slot
reuse used 0 CAS; live native credit remained retained until actual final free
and explicit flush. All 3 unit and 10 integration tests passed, as did locked
check, crate formatting, all-target/all-feature strict Clippy and the public
native example. The required actual default-feature Jemalloc server build
passed in 110.30 seconds. Actual nightly-2026-08-22 reports rustc
1.100.0-nightly (c656540d6), not an inferred 1.98.

Only the bridge package was cleaned at each source-variant switch, and both
old-red and current-green logs show actual recompilation. Cargo was released
immediately after the server build. Shared target/cache and unique proof remain
for independent reuse. The original bbcfbcd review remains failed; this repair
requires a new exact-head independent review and separate verification.
The unchanged 272-test native Chunk suite and original-versus-candidate cold
comparison are reserved for that verifier, not reported passed by this builder.
The earlier WouldBlock and stale-binary failures remain retained and unwaived.
No host/Arrow/SQL/E2E, memory, owner, bulk or merge gate is accepted.

The following observations describe the earlier implementation:

The public native example performs three actual selected-Jemalloc allocations,
retains 19632 native bytes while owners survive close, then leaves the 176-byte
static service credit after three physical frees and explicit refund.
The integration instance with two service slots retains 248 service bytes.
Cold callback reserve/close/release/denial each observed zero global allocations;
the selected-Jemalloc-forwarding allocating positive observed one.
Representative padded class charges match actual sallocx, including 100003 bytes
at alignment 128 becoming padded 100096 and charged 114688.

The required actual default-feature Jemalloc server build passed (67.32 seconds)
with the unchanged native warnings retained. Cargo was released after this final
command; the existing shared target is retained for independent reuse.
Independent fresh review and
verification, root integration, host Arrow engineering, production integrity
policy and all owner/memory/bulk-merge gates remain pending.

## Context and Orientation


The Rust workspace is `rust/`; native credit is implemented in
`rust/crates/tidb-memory/src/pool.rs`. A Budget owns reserved quota drawn from
a ResourcePool; it is not physical storage. The new
`rust/crates/tidb-tiforth-memory/` crate connects this credit to
`tiforth-memory` from owner Git revision
`9628339f697252d29eea885f2eac693f2673935d`. OwnedRoot is a physically allocated
header with reference-counted lifetime pins. PinnedAllocation owns initialized
payload bytes. PreparedMetadataSlab owns one physical header/control/payload
allocation whose published blocks can outlive the original query.

An envelope is payload capacity plus the full padded root header's Jemalloc
class. A service credit is a separate exact quota for static bridge storage and
the dependency's fixed slab identity atomic. A control operation may lock native
pools; a managed callback may only touch pre-admitted fixed atomics.

## Plan of Work


For the R1 correction, first edit only release handling in
`rust/crates/tidb-tiforth-memory/src/admission.rs`. Add test-only counting at
the actual strong-CAS boundary and real-root tests with three identical
envelopes; exercise the third live and closing root, duplicates and slot reuse.
Retain the same regression and counting transformation against the exact old
bbcfbcd release algorithm in private scratch. It must compile and fail at runtime
with more than two attempts; the repaired source must pass unchanged expectations.
Clean only the bridge package's generated artifacts before each source-variant
switch, so an older timestamp cannot reuse a stale negative binary.

First add the crate and workspace member, using actual tidb-memory and pinned Git
TiForth dependencies. Implement checked selected-allocator layout charging in
`src/allocator.rs`, compact errors in `src/error.rs`, finite native credit
ownership and nonblocking callbacks in `src/admission.rs`, and public wrappers
in `src/lib.rs`. The wrappers must derive every raw payload/slab/header charge.
Do not modify native pools, original tests, server dependencies, Go, or Bazel.

Then add public integration tests and an example using actual ResourcePool
budgets. Record allocator entries/frees independently from native allocated quota.
Use only an executable-local Jemalloc-forwarding observer for cold allocation
probes, including an allocating positive. In a separate derived scratch tree,
make only premature native refund wrong; the same retention assertion must fail
with exit 101 before any unsafe access/free. Return to the exact current source
and rerun the positive.

Finally run locked checks, tests and strict Clippy on the new crate, the original
272-test Chunk suite unchanged, and the required default-feature server build.
Only format new crate sources. Update this plan with actual results and limitations.

## Concrete Steps


From `rust/`, use the dispatch-provided existing CARGO_HOME and
CARGO_TARGET_DIR, CARGO_BUILD_JOBS=2, CARGO_INCREMENTAL=0,
RUST_MIN_STACK=33554432 and repository -Zthreads=8 flags throughout. Do not create
a second target directory. Generate Cargo.lock through Cargo, then run:

The host-specific cache locations must be supplied as CARGO_HOME and
CARGO_TARGET_DIR pointing at the existing host cache on mounted project storage;
do not create a replacement if project storage is absent. Set
CARGO_BUILD_JOBS=2, CARGO_INCREMENTAL=0 and RUST_MIN_STACK=33554432 in the shell.
The checked-in `rust/.cargo/config.toml` supplies -Zthreads=8. Install the exact
nightly toolchain plus rustfmt and clippy if not already present; do not change
the workspace toolchain pin or global default.

    cargo +nightly-2026-08-22 check --locked -p tidb-tiforth-memory --tests
    cargo +nightly-2026-08-22 test --locked -p tidb-tiforth-memory -- --test-threads=2
    cargo +nightly-2026-08-22 clippy --locked -p tidb-tiforth-memory --all-targets --all-features --no-deps -- -D warnings
    cargo +nightly-2026-08-22 run --locked -p tidb-tiforth-memory --example native_owned_root
    cargo +nightly-2026-08-22 nextest run --locked -p tidb-chunk --test-threads 2
    cargo +nightly-2026-08-22 build --locked -p tidb-server

For the R1 regression specifically, run the following from `rust/`:

    cargo +nightly-2026-08-22 test --locked -p tidb-tiforth-memory --lib last_equal_envelope_ -- --nocapture --test-threads=2

Expect two passing tests and actual_release_cas=1 for both live and closing
third equal envelopes. The retained source-bound old algorithm uses the
byte-identical test module and counting helper but exits 101 with actual counts
3 and 4. Its private transformation, exact source hashes, patch and raw logs
are referenced in the repair report; do not alter the original pinned source.

## Validation and Acceptance


The new tests must show denied capacity has no backend allocation entry, bytes
remain usable after root/query/close request, cross-thread last free precedes
root release and explicit Budget.clear, and service credit remains charged.
Duplicate/stale epochs, bounded slots, shutdown, contention and layout overflow
are explicit errors. All first-use managed operations must have zero observer
allocations while the allocating positive registers at least one. The negative
must fail at the identical native-retention assertion; compilation failure,
changed expectation or invalid pointer use is not evidence.

## Idempotence and Recovery


All checks are repeatable. A static service initializes only once per process;
tests use independent static instances. Forgotten owners conservatively retain
native quota. Failed root construction becomes eligible for explicit control
flush only after the allocator owns no block. Preserve unsuccessful raw logs.
Never reset Git or mutate the pinned original source to recover a failed check.

## Artifacts and Notes


The immutable dispatch binds host f3f5b267c41c9f6d2466baaccff714214893782f and
TiForth 9628339f697252d29eea885f2eac693f2673935d. The original native baseline
reported 272 passing Chunk tests and a default Jemalloc server build; these do
not replace executing modified-candidate checks. Private raw evidence and report
stay outside committed product files.

## Interfaces and Dependencies


NativeService stores fixed records, a selected NativeJemalloc instance and native
Budget ownership. initialize installs separately reserved service credit;
reserve_query registers an already reserved exact native envelope; try_root
constructs real OwnedRoot storage. request_close closes admission but cannot
refund live roots; flush performs once-only native clear only after release or
unopened cancellation. NativeRoot and NativeQuery expose checked raw payload and
prepared metadata operations while actual returned TiForth owners pin lifetime.

The concrete signatures are `NativeService::initialize(&'static self,
&mut Budget)`, `reserve_query::<Q,G,E>(&self, epoch, payload, &mut Budget)`,
`try_root::<Q,G,E>(&'static self, epoch)`, `request_close(&self, epoch)` and
`flush(&self, epoch)`. initialize/reserve_query transfer Budget ownership only
on success. NativeQuery::try_allocate accepts Layout and SiteId;
NativeQuery::prepare_metadata accepts MetadataPlan and SiteId.

Initial revision: documents-first bounded implementation plan.

Verification revision: implemented the public checked route, retained first
failures, demonstrated safe negative/current positive, and passed the original
Chunk suite without changing any native fixture or expectation.

Native-build revision: required modified-candidate default Jemalloc server build
passed. This is build evidence only; no server startup, Arrow route, SQL/E2E,
performance, production integrity policy or owner gate is claimed.

R1 revision: repaired the fixed total two-CAS bound without acceptance changes;
added real-root production-CAS and public native-credit regression coverage,
clarified truthful direct low-level callbacks, and retained old-red/current-green
proof. Only assigned bridge source/tests/docs changed; Cargo/lock/native core and
the pinned TiForth dependency are unchanged. Fresh exact-head independent review
and verification remain required.

Handoff revision: original 13156 tracked entries and Git index/config are
unchanged. Only the two allowed existing Cargo files changed; ten new product
files are additive. Raw negative proof, original native binary and shared target
are retained. Removed only the inactive derived negative source tree and own
temporary files; measured project free-space recovery was 343388160 bytes
(a net filesystem observation, not a universal per-file accounting claim).
