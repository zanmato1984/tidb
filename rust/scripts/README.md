# `rust/scripts`

Operational tooling for the Rust workspace: real-TiKV runners, benchmarks,
generators and audits.

## Private TiForth dependency credential helper

The native-memory workflow supplies its job token only to the locked metadata
step. `tiforth-dependency-credential.py` handles only `get` for HTTPS
`github.com/zanmato1984/tiforth` (with optional `.git`). It returns credentials
over Git's private helper pipe, never writes a credential store, and rejects
`store`/`erase`, unknown fields, malformed or duplicate identity fields, and
requests beyond the existing line/count bounds. The token is read only after
request validation and is unset before the identity guard runs.

Git's HTTP authentication path advertises `capability[]=authtype` and
`capability[]=state`; plain `git credential fill` does not add them by default.
The helper accepts exactly these known advertisements as bounded metadata, like
`wwwauth[]`, but does not echo capability support or accept negotiated
`authtype`, `credential`, `ephemeral`, `state[]`, or `continue` values. No
additional target, protocol, operation, or unknown capability is authorized.
See the [Git credential protocol](https://git-scm.com/docs/git-credential) and
[Git 2.56.1 HTTP implementation](https://github.com/git/git/blob/v2.56.1/http.c).

Run the Python controls without Cargo or real credentials:

```bash
python3 -B rust/scripts/test_check_tiforth_native_memory.py -v
```

The HTTPS regression requires installed Git and OpenSSL. It creates a tiny
synthetic repository and a one-use test certificate, and points Git only at a
loopback CONNECT fixture that terminates TLS locally and never forwards traffic.
It uses the exact HTTPS host/path request shape, exercises a real authenticated
Git fetch, verifies the synthetic object ID, and checks missing-helper,
missing-token and foreign-target failures. Its workflow-shell probe substitutes
Cargo with a mock that invokes real Git; it also asserts token removal before
the real metadata guard. Temporary repositories, certificate and server are
cleaned up. These controls are not a real Cargo run, Linux qualification, or
proof that a job token can fetch the private pinned dependency; those remain
separate required CI evidence.


## Build and test commands

Run Cargo from `rust/` so workspace configuration applies. Build/test
commands use `--locked`. Gate a crate's behavior with nextest (one process
per test); raw `cargo test` shares process globals between tests:

```bash
cargo nextest run --locked -p tidb-session -E 'test(<filter>)'
cargo nextest run --locked -p tidb-executor -E 'test(window_executor_source)'
```

`cargo build --locked -p tidb-server` builds only the server executable. The
cluster-session diagnostic is opt-in; its live runners enable `diagnostics`
explicitly. For manual use from `rust/`:

```bash
cargo build --locked -p tidb-server --features diagnostics --bin cluster-session-smoke
target/debug/cluster-session-smoke --pd <address> --schema <database> --sql '<statement>'
```

The feature only selects the diagnostic target; it adds no dependencies or
runtime behavior.

Integration suites use their crate's `tests/all.rs` aggregate (for example
`tidb-planner` and `tidb-txnkv` keep their primitive translations under
`primitives::`). Register a new suite there; intentional isolated targets
belong in Cargo.toml (`tidb-printer --test printer_contract` changes the
logger, `tidb-systimemon --test systimemon_source` starts a persistent
monitor). Live transaction/DistSQL cases run under `tidb-distsql` through the
`run-realtikv-*.sh` runners, which own cluster setup and cleanup; shared
fixtures stay under `difftests/transaction-tests/fixtures`.

Go cases and fixtures are the correctness reference. Do not add source-shape,
call-count, file-size or historical test-count gates. Gate test-only modules
at their parent declaration so server builds do not load them. Unicode
runtime tests use the checked-in Go fixture; when maintaining its table, run
from the repository root:

```bash
python3 rust/scripts/generate-go-simple-case.py --check
```

`go-name-audit.py` lists Rust items whose names do not follow their Go
counterparts.

## Shared SQL server checks

Both storage engines use the ordinary session and shared catalog. Use:

- `run-realtikv-convergence.sh` for SQL, joins, ordering and shared catalog DDL.
- `run-live-write-proof.sh` for binary prepared reads/writes and sysbench.
- `run-realtikv-catalog-load.sh`, `run-realtikv-ddl.sh` and
  `run-realtikv-ddl-notify.sh` for metadata and cross-node schema visibility.
- `run-realtikv-multi-statement-txn.sh` for transaction visibility and locking.
- `run-realtikv-region-retry.sh`, `run-realtikv-transport-retry.sh` and
  `run-realtikv-lock-recovery.sh` for storage recovery.

A runner passing alone does not establish that persistent connections,
leader transfer and blocked shutdown pass together.

## The vendored client

`sync-tikv-client-rs.sh` replaces `third_party/tikv-client-rs` with
`ngaut/client-rust` master plus `third_party/patches/tikv-client-rs`, and
regenerates its protobuf bindings. The tracked copy also carries edits made
in place that are not in the patch set, which a sync would discard: turn them
into patches before syncing. Name the synced commit in the commit message.

Parser and lexer comparisons share `difftest`: from `rust/`, run
`cargo nextest run --locked -p difftest`. Source-inventory
generation/checking is an explicit maintenance operation; see
[the inventory workflow](../difftests/INTEGRATION_PLAN_INVENTORY.md).
