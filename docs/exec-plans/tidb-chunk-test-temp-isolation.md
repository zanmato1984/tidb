# Isolate Chunk test temporary storage by process

This ExecPlan is a living document. Maintain it according to repository-root `PLANS.md`; keep Progress, Surprises & Discoveries, Decision Log, and Outcomes & Retrospective current.

## Purpose / Big Picture


Concurrent native test processes must spill and read real Chunk data without sharing the process-wide temporary-storage directory. The fixture must still leave its global directory absent until the first real spill acquires the normal kernel lock. A separate child deliberately requesting an occupied directory must still receive WouldBlock. Production initialization, encryption, quotas, allocation and locking are unchanged.

## Progress


- [x] Read the owning Go package, Rust disk/config implementation, and fixture routes.
- [x] Add one process-lifetime owned parent and once-only configuration publication to the test fixture.
- [x] Add one real subprocess regression with explicit IPC barriers and known-value spill/read/close workloads.
- [x] (2026-11-16) Old-fixture runtime red exited 101 after real I/O and lock rejection; restored corrected helper forced a leaf recompile and identical regression passed.
- [x] (2026-11-16) Scoped formatting and corrected check passed. Strict all-target Clippy ran and FAILED on 15 unchanged original-test diagnostics; no lint suppression or original-test changes. The once-only owning suite passed 273/273, zero skipped, two threads, retries zero.
- [x] (2026-11-16) Recorded raw command/source hashes, original272 byte/name preservation, actual 273-test graph and exact pinned sources. Independent integrated-head review/verification remains outstanding.

## Surprises & Discoveries


The existing fixture isolates explicit SpillStorage paths, but DataInDiskByChunks::init_disk_file first calls tidb_disk::check_and_init_temp_dir using the global configuration. The default global path is not process-unique. The Rust and Go check-and-initialize APIs skip initialization when that path already exists; the regression therefore checks actual path separation after real I/O rather than asserting an unobserved historical race.

The private build overlay must include workspace manifests for implicit path members, not only the explicit member list. Otherwise Cargo either finds two tidb-chunk package identities or fails to resolve an implicit member. Unmodified dependency source entries remain linked to the exact read-only source tree; this is derived-builder evidence, not integrated-head qualification.

## Decision Log


- Decision: Publish only temp_storage_path once through config::update_global, guarded by OnceLock in the test-only fixture.
  Rationale: All ordinary fixture routes funnel through isolated_storage_with_quota. No test-wide serialization or repeated global publication is needed.
  Date/Author: 2026-11-16, bounded builder.
- Decision: Retain a tempfile::TempDir parent in a static OnceLock and configure its not-yet-created global child.
  Rationale: Atomic random directory creation provides collision resistance while preserving the cold production initializer.
  Date/Author: 2026-11-16, bounded builder.
- Decision: Use a single new test that recursively launches the same native test executable with an exact filter.
  Rationale: Every child executes a real workload or real adverse lock control; no ignored/no-op child helper is counted as coverage.
  Date/Author: 2026-11-16, bounded builder.

## Outcomes & Retrospective


The bounded builder observed red exit 101 for two shared cold global paths after both real spill/read/close workloads and same-path WouldBlock. The identical new regression then passed against a force-recompiled corrected helper with distinct cold paths. The corrected nextest suite ran once: 273 passed, zero skipped, including all 272 unchanged originals and one new test. Scoped rustfmt and cargo check passed. Strict all-target Clippy failed with 15 diagnostics in unchanged original test files; it is not reported as passed. Builder evidence alone does not approve, independently review, qualify, or accept an integrated head. The original historical failure remains preserved; no exact historical competing PID/syscall is claimed.

## Context and Orientation


rust/crates/tidb-chunk/src/lib.rs owns the cfg(test) storage fixture. storage and isolated_storage both eventually call isolated_storage_with_quota. rust/crates/tidb-chunk/src/chunk_in_disk.rs owns DataInDiskByChunks; its first add calls the production disk initializer before opening an explicit spill file. rust/crates/tidb-disk/src/temp_dir.rs owns the process-global kernel-lock handle. pkg/util/disk/tempDir.go is the corresponding Go semantic source. rust/crates/tidb-config/src/config_tree/config.rs publishes immutable global configuration snapshots under a write lock.

A cold path is a configured path that does not yet exist. An IPC barrier is an explicit line exchanged through child stdin/stdout; it orders operations without timing sleeps. The holder remains alive with its global lock while the contender spills on its separate global path. The same-path child requests the holder's path through initialize_temp_dir and must fail with WouldBlock.

## Plan of Work


In the cfg(test) module in lib.rs, replace the obsolete serial-lock comment and configure the global path before constructing any storage authority. Keep the owned parent alive and never pre-create its global child. Do not change the existing explicit SpillStorage path, encryption, quota or assertions.

Add src/test_temp_storage_process_tests.rs and register it only under cfg(test). In its one test, create a common suite temporary directory and launch holder and contender processes. Both report absent global paths before either is allowed to spill. Holder spills first and retains the kernel lock, then contender writes and reads 1,024 known int64 values and closes its real spill file while holder remains alive. A third child must reject the holder's real path. Finally require distinct global paths within the common suite directory and release the children. A 30-second IPC receive deadline bounds failure; process RAII kills/reaps children if a parent assertion fails.

Add only tidb-config and the already locked workspace tempfile as dev-dependencies. Cargo.lock may change only the tidb-chunk dependency list.

## Concrete Steps


From repository root, retain the source and use a private derived overlay for a bounded builder; never write a source checkout. Preserve all original 272 tests unchanged. In rust, use the existing target/cache with CARGO_BUILD_JOBS=2, CARGO_INCREMENTAL=0, RUST_MIN_STACK=33554432, the pinned nightly toolchain, and existing -Zthreads=8.

    cargo check -p tidb-chunk --tests
    rustfmt --edition 2021 --config skip_children=true,reorder_modules=false crates/tidb-chunk/src/lib.rs crates/tidb-chunk/src/test_temp_storage_process_tests.rs
    cargo clippy --locked -p tidb-chunk --all-targets --no-deps -- -D warnings

For the red control, retain the complete old lib.rs helper with only the new cfg(test) module declaration added; keep the new regression and dev-dependencies identical. Run the exact new test once. Expect compilation success and a runtime isolation assertion failure with exit 101 after real I/O and lock rejection. Retain raw logs and exact source hashes. Restore the corrected lib.rs and force the owning leaf's recompilation by updating its mtime, without purging the cache.

    cargo test --locked -p tidb-chunk --lib test_temp_storage_process_tests::process_isolated_fixture_spills_and_preserves_lock_rejection -- --exact --nocapture

Expect the corrected targeted regression to pass and the same-path child to report WouldBlock. Only after the targeted proof, run one full owning suite with a fresh TMPDIR and no retries:

    cargo nextest run --locked -p tidb-chunk --test-threads 2 --retries 0

Expect 273 tests: all original 272 plus the new regression. Do not rerun the full suite to turn a failure green. Record actual failures and unrun checks instead.

## Validation and Acceptance


The behavior proof must show cold initial global paths, actual nonempty spill files, all known values read back, spill-file removal on close, holder liveness during contender I/O, distinct process global paths, and genuine same-path rejection. The old fixture must fail the same compiled regression at runtime. All original test files and test counts remain unchanged except the added module. Formatting is scoped to the two changed Rust source files.

Strict Clippy may expose existing host warnings; preserve and report them, do not broaden the patch or suppress lint rules to hide them. The builder's check and suite apply only to its recorded derived identity. Independent review and separate verification, including default server build, remain required on the integrated head.

## Idempotence and Recovery


Use unique proof names and fresh TMPDIRs so failed attempts remain immutable. Keep old-fixture and corrected source snapshots and hashes. A failed subprocess is killed and reaped; no sleep, retry, prewarm, skip or altered thread count may replace a meaningful proof. Do not change production lock behavior to repair tests. Restore only the private overlay; root alone integrates collected products.

## Artifacts and Notes


The assigned report records every native command, actual exit, raw proof hash, exact source combination, derived manifest/config/lock identity and final resource release. No ticket, token, secret or machine-specific path belongs in repository products. Private proof paths remain in the fenced report only.

## Interfaces and Dependencies


The fixture uses tidb_config::config_tree::config::update_global only from cfg(test), and tempfile::Builder::tempdir for an atomically owned unique parent. DataInDiskByChunks::add/get_chunk/close and tidb_disk::initialize_temp_dir/clean_up are the unchanged real native interfaces exercised by the regression.

Revision note: initial bounded implementation and validation procedure, 2026-11-16.


Revision note: recorded actual red/green, once-only suite and preserved strict-Clippy failure; independent integrated-head review and verification remain mandatory.
