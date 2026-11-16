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

The actual Linux run at host `4aaccff549bf86b273e2ad2043e4aa47d0a8f73e`
then failed fetching private TiForth with the public host's built-in token;
the public failure remains a failure. Public evidence is retained at
https://github.com/zanmato1984/tiforth/issues/7#issuecomment-6923703769.
The bounded correction makes this same native workflow reusable by an own-repo
pull request changing the private TiForth caller file, or by an already-registered
manual workflow in private `zanmato1984/tiforth`. GitHub associates a reusable run with
the caller, so the existing caller `contents: read` `GITHUB_TOKEN` can read its
own private repository. This is not a new credential or grant and cannot give
the public host's direct token access. See GitHub's primary contracts:
https://docs.github.com/en/actions/concepts/security/github_token and
https://docs.github.com/en/actions/reference/workflows-and-actions/reusing-workflow-configurations.

## Progress


- [x] (2026-11-16) Read exact source `bbcfbcdff7c8460a0c2ba1283d0324f91eefdf9b`, repository instructions, toolchain, manifests, workflows, and retained native dependency evidence.
- [x] (2026-11-16) Fix scope and prepare this documentation before implementation.
- [x] (2026-11-16) Add read-only workflow and production dependency/allocator identity guard.
- [x] (2026-11-16) Pass 20 Python tests, actual workflow-shell failure probes, and Ruby/Psych YAML syntax parsing; no Rust build/test was run by this builder.
- [x] (2026-11-16) Under a later metadata-only resource grant, retain the offline Cargo failure, then obtain real locked metadata on the unchanged base and pass the production guard plus nine metadata corruption controls.
- [x] (2026-11-16) Root integrated source `29073fbc4e34a82a1e0176143adf3a119db29ef5`; independent product review and 12 local checks cover only that exact source.
- [x] (2026-11-16) Root published PR #17; remote run 43905819653 failed workflow validation before creating any jobs. DCO passed separately; Linux native CI did not run.
- [x] (2026-11-16) Repair only invalid job-level runner context. actionlint 1.7.12 rejects all three original expressions and accepts the repair; 23 Python tests pass, including all 20 original tests and actual setup-block probes. The new runtime regression fails against the original setup block. No Cargo/native builds were run by this repair builder.
- [ ] Fresh independent review and verification, including the required default server build, cover the repaired exact head.
- [ ] Root publishes the repaired head and observes actual remote Linux CI; owner gates remain open.
- [x] (2026-11-16) Document the actual public-host private-Git authentication failure before preparing the bounded reusable-workflow correction.
- [x] (2026-11-16) Retain the initial manual-only seven-product staged correction and its 35+7 local test results. It cannot bootstrap an unregistered workflow and is superseded by the bounded PR-launch correction below.
- [x] (2026-11-16) Under the v2 envelope, add only own-repository private PR opened/synchronize/reopened events filtered to the caller workflow file; retain optional registered dispatch. Actual Bash context/checkout controls, 36 host tests, 8 explicit-host private tests and both actionlint schema/context checks pass locally. Under v3's isolated toy-Git allowance, the full private baseline 116, default combined 120 and explicit-host combined 124 tests pass on local Python 3.11.15; declared CI Python 3.12 remains unrun locally.
- [ ] Root replaces both zero-SHA staging placeholders with the actual final host commit, then obtains fresh combined-head independent review and verification, the default Jemalloc server build, and actual private-caller Linux results.

## Surprises & Discoveries


A newly introduced `workflow_dispatch`-only file cannot launch until the
workflow exists on the default branch. The original staged manual-only version
and checks are retained as superseded local evidence, not executable remote
qualification. GitHub documents this at
https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_dispatch.
The effective v2 task envelope permits a narrowly filtered same-repository
private PR trigger instead; no merge or default-branch change is authorized
merely to register the workflow. Actual GitHub execution remains unrun.

Actual local Git credential IPC terminates helper input at EOF after the last
newline, not necessarily with a blank line. Also, a credential URL section for
`.../tiforth` does not match the `.git` suffix. Initial fake-only protocol tests
failed for both reasons; their raw failures are retained separately. The helper
now accepts a complete newline-delimited request ending at EOF or a blank line.
Process-only helper configuration applies the helper, whose own protocol/host/
exact-path allowlist remains the sole credential release check. Both exact path
variants pass through real Git IPC, and other targets remain rejected.

An isolated-HOME metadata-shell test initially could not import the already
installed `tomli` on the local Python 3.9 interpreter. The test now passes only
that existing parser's import directory to its clean environment; it installs
nothing and does not restore desktop HOME or credentials. Ubuntu 24.04's Python
has standard-library `tomllib`. Initial checker fixture errors and the omitted
unchanged guard in the seven-product output tree are retained as local test
failures, not reclassified as remote CI failures.

The first actual remote run of PR #17,
https://github.com/zanmato1984/tidb/actions/runs/43905819653, rejected
`${{ runner.temp }}` in the three job-level environment values at lines 41-43.
No jobs were created. The formal finding is
https://github.com/zanmato1984/tidb/pull/17#pullrequestreview-5923389637.
YAML parsing and shell probes had passed locally but do not validate GitHub's
context-availability rules. DCO passed independently; it is not a Linux CI pass.
Prior review and the 12 local acceptance checks apply only to
`29073fbc4e34a82a1e0176143adf3a119db29ef5`, never the repaired head.

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


Decision: Correct initial manual-only launchability with narrowly scoped
private `pull_request` events (`opened`, `synchronize`, `reopened`) whose path
filter is only `.github/workflows/host-native-memory.yml`. Keep optional
`workflow_dispatch`, documenting that it needs default-branch registration.
Rationale: an unmerged draft PR can qualify the new caller without a default
branch merge, write-token permission, secret inheritance or native-check
change. The callee rejects foreign/fork PR heads rather than skipping checks.
Date/Author: 2026-11-16, effective v2 bounded task envelope.

Decision: Add `workflow_call` without replacing the own-host pull-request,
push, or manual triggers, and keep every native command in this one workflow.
Validate the allowed repository, event, caller workflow reference and nonzero
40-hex source pin before checkout; checkout always names `zanmato1984/tidb`,
then verify its actual HEAD before Cargo. Own-host pull requests must originate
in the own host, not another repository. A private caller must be the exact
`zanmato1984/tiforth/.github/workflows/host-native-memory.yml` workflow and must
provide an explicit source pin. Under v2 it may be a registered manual branch
dispatch or a pull request with a same-repository head, numeric merge ref and
opened/synchronize/reopened action. Foreign/fork heads, mismatched refs and
other event types fail before credential use. Rejection fails the job, rather than producing
a skipped green check. Rationale: reusable workflows retain caller context,
so `github.sha` must never silently select TiForth as host source.
Date/Author: 2026-11-16, isolated private-caller CI builder.

Decision: Give only the locked metadata step the caller's existing token.
Cargo's CLI Git fetch receives process-environment Git configuration with an
empty helper reset, `credential.useHttpPath=true`, and the checked-in Python
helper. The helper answers only `get` for HTTPS `github.com` and the exact
`zanmato1984/tiforth` or `.git` path; all other requests fail without credentials.
Its stdout is Git's credential-protocol pipe, never a log. No token is placed
in a URL, file, Git configuration, `GITHUB_ENV`, cache, artifact or later build
step. Shell-local configuration and token variables are unset on exit.
Rationale: fetch the unchanged real Git revision without new credentials or
desktop authentication. Date/Author: 2026-11-16, isolated CI builder.

Decision: Stage the not-yet-created host callee and source as exactly forty
zeroes in the private caller, with explicit rejection by final qualification.
Root must commit the host and replace both positions with that same immutable
commit before dispatch. Staging checks are not final qualification or a Linux
pass. No action is dispatched by this builder. Date/Author: 2026-11-16.

Decision: Initialize `CARGO_HOME`, `CARGO_TARGET_DIR`, and `NATIVE_MEMORY_LOGS`
from runtime `RUNNER_TEMP` in the existing Prepare diagnostics step, export
them immediately, and append their concrete values to `GITHUB_ENV` for later
steps. Rationale: the runner context is unavailable in job-level `env`;
`GITHUB_ENV` alone does not affect the currently executing step. This keeps all
three paths runner-temporary without changing locked commands or source.
Date/Author: 2026-11-16, isolated CI repair builder.

Decision: Validate this workflow with real actionlint 1.7.12 context/schema
checking as well as YAML parsing and actual Bash execution. Obtain its prebuilt
binary from the official rhysd/actionlint release and verify both the release
asset digest and the published checksum list before executing it. Rationale:
the actual failure was a platform context rule, not a YAML or shell syntax
error. Date/Author: 2026-11-16, isolated CI repair builder.

Historical decision (superseded only by the explicitly bounded private caller
above): Only the own-fork repository `zanmato1984/tidb` runs this job; checkout
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


The corrected private-caller candidate passes 36 host and 8 explicit-host private Python tests,
including the unchanged original 23 tests, actual caller/source Bash execution,
wrong checkout and Git-failure rejection, the failure-masking mutant, helper and
real Git credential-fill fake-token controls, and the actual metadata block
under process-only credential configuration. No sentinel enters test diagnostics,
artifact logs, `GITHUB_ENV`, global config or any sandbox file. Both workflows
pass actionlint 1.7.12. Final qualification rejects the staged zero pin; actual
host commit resolution, independent final-head review/verification, required
default server build and actual private Linux remain unrun. No claim about
real private-Git authorization is made from these local controls. The original
manual-only 35+7 checks remain superseded evidence, not a new workflow-launch
claim. All 116 original private workflow tests and the combined ordinary 120
and explicit-host 124 tests also pass, without skipping the existing snapshot
test. That test's toy Git init/add/commit/archive is confined to isolated scratch
under the v3 verification allowance; real repositories remain unchanged. New
private tests use only stdlib; local Python is 3.11.15, while actual declared
CI Python 3.12 execution remains unrun.

The original four scoped products were implemented. Twenty Python tests passed, including
execution of actual workflow blocks with failing mock Cargo commands and real
production-guard rejection; YAML parsing succeeds. The actual locked base-source
metadata passes the same guard, and nine deliberate metadata corruptions fail.
The first unit run exposed a test fixture that edited an unrelated lock entry;
the corrected fixture names the allocator entry explicitly, and the original
failure is preserved. No live CI, Rust build/test, combined-head verification,
independent review, or owner gate has passed as a result of this builder work.
The separate native admission repair owns production code; this automation does
not alter its fixed two-CAS bound.

The later CI-only repair changes this workflow, its existing Python tests, and
this plan. actionlint 1.7.12 reports exactly three disallowed `runner` contexts
at original lines 41-43 and exits 1; the repaired workflow exits 0. All 20 prior
tests plus three runtime environment tests pass (23 total). The new positive
environment test fails on the original setup block. The runtime probes execute
the actual setup block using a fresh temporary root containing spaces and a
clearly synthetic checkout identity. They verify immediate exported values in
a child process, concrete `GITHUB_ENV` records loaded for the next step, three
isolated directories and diagnostic logs, and nonzero exits on missing runner
temp or failed environment persistence. These are local CI-control results,
not Rust execution or remote Linux success. Fresh independent review,
new-head verification, default server build, and actual remote CI are pending.

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

The repair milestone removes only the three invalid job-level temporary-path
values. `Prepare diagnostics` exports them from `RUNNER_TEMP`, appends concrete
values to `GITHUB_ENV`, then creates their directories before logging. Exports
make the values immediately visible to this step's subprocesses; `GITHUB_ENV`
is the runner's line-based environment handoff to subsequent steps, not a
shell script to expand later. The artifact upload's step-level `runner.temp`
expression remains valid. The three existing job-level resource limits and all
locked native commands, defaults, permissions, filters, and failure behavior
remain unchanged.

## Concrete Steps


From the repository root, run the non-Cargo checks:

    PYTHONDONTWRITEBYTECODE=1 python3 -B rust/scripts/test_check_tiforth_native_memory.py -v
    PYTHONDONTWRITEBYTECODE=1 python3 -B rust/scripts/check-tiforth-native-memory.py --workspace rust --source-only
    actionlint -shellcheck= -pyflakes= .github/workflows/tiforth-native-memory.yml

The repair used pinned actionlint 1.7.12 from
https://github.com/rhysd/actionlint/releases/tag/v1.7.12, installed only in
private task scratch, not globally. Download the matching official prebuilt
archive and checksum file; verify the archive's SHA-256 against both the release
asset digest and the checksum file before extraction. `actionlint -version`
must identify 1.7.12. The two empty optional linter flags disable integrations
with external shellcheck/pyflakes executables, not actionlint's YAML/schema or
GitHub context checks. Run against only this workflow; legacy workflows are
outside this repair. Expect no actionlint diagnostics and 23 passing Python
tests. On the original exact `29073fbc4e34a82a1e0176143adf3a119db29ef5`
workflow, actionlint must exit nonzero for the three job-level `runner` contexts.

Python 3.11 or later uses standard-library `tomllib`; older Python 3.9/3.10 needs
already-installed `tomli`. Ubuntu 24.04's Python provides `tomllib`. The test
suite needs Bash and locates the checked-in workflow relative to its script.

From `rust/`, with required native tools and nextest installed, execute:

    cargo +nightly-2026-08-22 metadata --locked --format-version 1 > native-memory-metadata.json
    PYTHONDONTWRITEBYTECODE=1 python3 -B scripts/check-tiforth-native-memory.py --workspace . --metadata native-memory-metadata.json
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
that GitHub has run the job. Real actionlint schema/context checks must reject
the exact original workflow and accept the repaired workflow. Every actual
literal run block must also pass `bash -n`. The environment regression must
execute `Prepare diagnostics`, verify immediate exports and next-step records
without recursive shell expansion, and reject setup failures. These checks
complement, rather than replace, the original Cargo/tee, second-doctest, and
production-guard failure propagation tests.

Fresh combined-head verification must run actual metadata, all listed Rust
commands, and independent review. Remote CI acceptance requires an observed run
at that exact published head. Any cold test failure remains a failure with its
original logs; no retry can retrospectively turn the first run green. Full host,
corpus, Arrow, RSS, SQL/E2E, performance, bulk-autonomy, merge, and owner gates
remain outside this workflow's claim.

For the cross-repository correction, first run the host Python suite (the
original 23 tests remain), actionlint 1.7.12 on both workflows, and `bash -n`
on every actual literal run block. Run the private control suite with
`TIFORTH_HOST_NATIVE_WORKFLOW` naming the exact retained host workflow. The
suite executes the real context/source checks and Git credential helper using
only fake sentinel values, isolated HOME/config, no network, and real
`git credential fill` subprocesses. Positive credential stdout is captured
privately by the test process; diagnostics must not contain the sentinel.
Missing helper/token, wrong target/action/protocol, malformed input, wrong
caller/source checkout, and failure-masking controls must fail.

Then run `python3 -B workflow/test_host_native_memory_workflow.py --qualify`
with the same host-workflow environment. It must reject an unresolved zero
callee/source pin. After root materializes a real host commit, the two pins
must be identical, full nonzero 40-hex values and the action/callee references
must remain SHA-pinned. Local fake protocol success cannot prove that GitHub
will authorize the actual private Git fetch. Fresh final-commit review,
verification/default server build, and actual private Linux are still required;
the old failed public run is never relabeled passed.

## Idempotence and Recovery


Checks are repeatable and read-only with respect to tracked source. Each runner
uses temporary Cargo home/target paths and does not restore a cache. A failed
job uploads bounded diagnostic logs, not binaries or credentials. Diagnose and
fix actual failures in a new reviewed candidate; do not weaken flags or defaults.
No deployment or external write step exists. Root alone integrates products;
the private-caller builder writes only its seven assigned product paths and private
report in fenced output, never the source snapshot.

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

Revision note (2026-11-16): Record actual zero-job remote workflow validation
failure and the distinct passing DCO/local-290 evidence; repair runtime-only
temporary-path setup, add schema and environment regression evidence, and
retain new-head review/verification/remote-CI obligations without weakening
native acceptance.

Revision note (2026-11-16, private caller): Add documented existing-token
cross-repository orchestration only. Preserve real Git TiForth
`9628339f697252d29eea885f2eac693f2673935d`, all 13 native Rust products,
lockfile, nightly/resource limits and all required native checks. No memory
architecture, dependency, allocator, threshold, visibility or repository-setting
change is authorized here. Framework/memory/owner gates remain open.
