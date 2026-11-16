# Validate the native TiForth memory bridge in read-only CI

This ExecPlan is a living document. Maintain it according to root `PLANS.md`,
including Progress, Surprises & Discoveries, Decision Log, and Outcomes &
Retrospective. The tracked prerequisite is
https://github.com/zanmato1984/tiforth/issues/7, related to memory issue #1.

## Purpose / Big Picture


Contributors need a reproducible Linux check of the additive native memory
bridge, not an inference from a different repository's CI. After this change,
an own-fork pull request, task-branch push, or manual workflow run can check the
real Git TiForth dependency, selected native allocator, bridge tests, public
example, unchanged native Chunk suite, and default server build. A failing
command fails the job and leaves its logs available for diagnosis. A green run
does not establish server startup, SQL execution, end-to-end behavior,
performance, process-wide memory admission, or owner acceptance.

## Progress


- [x] (2026-11-16) Read exact source `bbcfbcdff7c8460a0c2ba1283d0324f91eefdf9b`, repository instructions, toolchain, manifests, workflows, and retained native dependency evidence.
- [x] (2026-11-16) Fix scope and prepare this documentation before implementation.
- [x] (2026-11-16) Add read-only workflow and production dependency/allocator identity guard.
- [x] (2026-11-16) Pass 20 Python tests, actual workflow-shell failure probes, and Ruby/Psych YAML syntax parsing; no Rust build/test was run by this builder.
- [x] (2026-11-16) Under a later metadata-only resource grant, retain the offline Cargo failure, then obtain real locked metadata on the unchanged base and pass the production guard plus nine metadata corruption controls.
- [ ] Root integrates automation with the separate native admission repair into a new exact candidate.
- [ ] Fresh independent review and actual locked Rust verification cover that combined candidate.
- [ ] Root publishes an own-fork branch and observes an actual remote CI run.

## Surprises & Discoveries


The immutable source has seven workflow files, not the five described in the
initial discovery summary. None invokes Cargo. The Rust toolchain is pinned to
`nightly-2026-08-22`; `.cargo/config.toml` supplies `-Zthreads=8` and a 32 MiB
test-thread stack. Workspace profiles already bound debug information, so this
workflow does not introduce a different profile.

Initially retained native evidence was `cargo tree` output, not format-1 Cargo
metadata. The controller subsequently granted a narrow metadata-only slot. The
offline command failed because `android_system_properties v0.1.5` was not cached;
that failure remains retained. One separately authorized online locked metadata
command then succeeded, and the production guard passed against that real
report on unchanged source `bbcfbcdff7c8460a0c2ba1283d0324f91eefdf9b`. Nine
independent corruptions of that actual report were rejected. Unit-test fixtures
remain explicitly synthetic. Fresh metadata on the final combined source
remains a separate verifier obligation. A prior
native Chunk run had a cold file-I/O failure; this workflow must expose such
failures and must not prewarm, retry, filter out, or skip them.

## Decision Log


Decision: Only the own-fork repository `zanmato1984/tidb` runs this job; checkout
uses the event's exact head, with credentials not persisted and only read
permissions. Rationale: source validation requires neither upstream mutation nor
privileged execution. Date/Author: 2026-11-16, CI builder.

Decision: Check only the relevant dependency edges, source manifests, and lock
entries. Rationale: Cargo metadata describes the large host workspace; proving
the native bridge and server allocator identities is not a claim that the whole
workspace is a closed dependency graph. Date/Author: 2026-11-16, CI builder.

Decision: Use pinned nextest 0.9.152, the host's existing test runner, for complete
bridge and Chunk test suites with two concurrent tests and no retries. Rationale:
process-isolated host tests should use the native playbook without concealing
first-use failures. Date/Author: 2026-11-16, CI builder.

## Outcomes & Retrospective


The four scoped products are implemented. Twenty Python tests pass, including
execution of actual workflow blocks with failing mock Cargo commands and real
production-guard rejection; YAML parsing succeeds. The actual locked base-source
metadata passes the same guard, and nine deliberate metadata corruptions fail.
The first unit run exposed a test fixture that edited an unrelated lock entry;
the corrected fixture names the allocator entry explicitly, and the original
failure is preserved. No live CI, Rust build/test, combined-head verification,
independent review, or owner gate has passed as a result of this builder work.
The separate native admission repair owns production code; this automation does
not alter its fixed two-CAS bound.

## Context and Orientation


The Rust workspace lives in `rust/`. Its additive package
`crates/tidb-tiforth-memory` connects an already-reserved native `Budget` to
TiForth-owned storage through selected Jemalloc allocation functions. Its public
example is `examples/native_owned_root.rs`. A Cargo package is a manifest-named
Rust build unit; its tests do not replace tests of the server process.

`rust/crates/tidb-chunk` owns the original native Chunk container suite.
`rust/crates/tidb-server/Cargo.toml` declares `default = ["jemalloc"]`; that
feature selects optional Jemalloc dependencies and profiling. A feature is a
named Cargo dependency/configuration switch. The server's binary installs the
selected allocator under this feature. The bridge remains additive and is not a
server integration or Arrow route.

`rust/Cargo.lock` fixes TiForth at Git revision
`9628339f697252d29eea885f2eac693f2673935d`, `tikv-jemallocator` at `0.7.0`, and
`tikv-jemalloc-sys` at
`0.7.1+5.3.1-0-g81034ce1f1373e37dc865038e1bc8eeecf559ce8`. Cargo's `--locked`
option rejects a command that would need to change that lock. Format-1 metadata
is Cargo's JSON package and dependency-resolution report, including activated
features. It is not memory-admission evidence.

## Plan of Work


Milestone one adds `rust/scripts/check-tiforth-native-memory.py`. It reads the
source manifests and lockfile, then checks fresh metadata's relevant resolved
edges. It rejects a path copy masquerading as TiForth, a wrong Git revision,
wrong native allocator versions, missing profiling, or a changed server default.
Its explicit source-only mode checks declarations but cannot claim resolution.

Milestone two adds `rust/scripts/test_check_tiforth_native_memory.py`. Tests call
the production guard with valid and deliberately corrupted inputs. Shell probes
execute the workflow's actual validation blocks with a failing mock Cargo command
and verify that failures survive logging pipelines. Mocks are shell-control
tests, never evidence of successful Cargo execution.

Milestone three adds `.github/workflows/tiforth-native-memory.yml`. Ubuntu 24.04
installs native prerequisites, the exact nightly with rustfmt/Clippy, and pinned
nextest. Build-essential supplies the native compiler/make used by Jemalloc;
pkg-config/libssl-dev serve the server's OpenSSL dependency; cmake serves the
locked aws-lc-sys dependency; protobuf-compiler supplies `protoc` called by
`rust/crates/tidb-proto/build.rs`. Cargo jobs and nextest threads are two,
incremental compilation is disabled, and native frontend/stack settings remain.
There is no persistent cache or build-artifact upload; only logs/metadata are
uploaded with seven-day retention, including on failure.

## Concrete Steps


From the repository root, run the non-Cargo checks:

    python3 rust/scripts/test_check_tiforth_native_memory.py -v
    python3 rust/scripts/check-tiforth-native-memory.py --workspace rust --source-only

Python 3.11 or later uses standard-library `tomllib`; older Python 3.9/3.10 needs
already-installed `tomli`. Ubuntu 24.04's Python provides `tomllib`. The test
suite needs Bash and locates the checked-in workflow relative to its script.

From `rust/`, with required native tools and nextest installed, execute:

    cargo +nightly-2026-08-22 metadata --locked --format-version 1 > native-memory-metadata.json
    python3 scripts/check-tiforth-native-memory.py --workspace . --metadata native-memory-metadata.json
    cargo +nightly-2026-08-22 check --locked -p tidb-tiforth-memory --all-targets --all-features
    cargo +nightly-2026-08-22 fmt --package tidb-tiforth-memory -- --check
    cargo +nightly-2026-08-22 clippy --locked -p tidb-tiforth-memory --all-targets --all-features --no-deps -- -D warnings
    cargo +nightly-2026-08-22 nextest run --locked -p tidb-tiforth-memory --all-features --test-threads 2 --no-fail-fast --retries 0
    cargo +nightly-2026-08-22 test --locked -p tidb-tiforth-memory --all-features --doc
    cargo +nightly-2026-08-22 run --locked -p tidb-tiforth-memory --example native_owned_root
    cargo +nightly-2026-08-22 nextest run --locked -p tidb-chunk --test-threads 2 --no-fail-fast --retries 0
    cargo +nightly-2026-08-22 build --locked -p tidb-server

Set `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, and
`RUST_MIN_STACK=33554432` for these commands. Do not replace native config's
`-Zthreads=8`, opt out of server defaults, substitute System for Jemalloc, or
run a prewarming/retry workaround. The last command builds, but does not start,
the server. Keep raw stdout/stderr and exact source identity for every command.

## Validation and Acceptance


The Python tests must report success for a coherent fixture and reject each
independent corruption, through the same production functions and CLI used in
CI. A source-only success must clearly say that no metadata was checked. The
workflow-shell probes must observe a nonzero exit when Cargo fails even though
`tee` succeeds, and when the real guard rejects otherwise successfully emitted
metadata. YAML must parse with an available real parser; parsing is not proof
that GitHub has run the job.

Fresh combined-head verification must run actual metadata, all listed Rust
commands, and independent review. Remote CI acceptance requires an observed run
at that exact published head. Any cold test failure remains a failure with its
original logs; no retry can retrospectively turn the first run green. Full host,
corpus, Arrow, RSS, SQL/E2E, performance, bulk-autonomy, merge, and owner gates
remain outside this workflow's claim.

## Idempotence and Recovery


Checks are repeatable and read-only with respect to tracked source. Each runner
uses temporary Cargo home/target paths and does not restore a cache. A failed
job uploads bounded diagnostic logs, not binaries or credentials. Diagnose and
fix actual failures in a new reviewed candidate; do not weaken flags or defaults.
No deployment or external write step exists. Root alone integrates the four
products; the builder's fenced output never mutates the source snapshot.

## Artifacts and Notes


Local builder proof is reported separately, with actual command exits and hashes.
No private host paths, worker tickets, or raw controller state belong in these
committed products. Test fixtures are synthetic unless explicitly identified as
the current source's real manifests/lock. The workflow's metadata and log archive
will be the evidence of a future run, not an assumed present result.

## Interfaces and Dependencies


The guard CLI accepts `--workspace PATH` plus exactly one of `--metadata JSON`
or `--source-only`. Success prints a JSON identity summary and exits zero;
rejection prints a concise diagnostic and exits nonzero. `check_source` and
`check_metadata` are the production functions exercised by tests. No Rust API,
Cargo manifest, lockfile, host allocator, or runtime behavior changes here.
