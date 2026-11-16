# tidb-tiforth-memory

Callable additive native pre-reserved Budget -> TiForth OwnedRoot and prepared
metadata -> selected Jemalloc physical allocation bridge. See
`../../docs/tiforth-native-memory.md` for exact boundary and lifecycle contract.

Reserve native service_bytes separately and call initialize on a genuine static
NativeService. Reserve exact root_request.total_bytes as a native Budget and
transfer it with reserve_query. try_root allocates the actual root header only
after admission. NativeRoot opens NativeQuery; its checked try_allocate and
prepare_metadata derive padded selected Jemalloc size-class charges.

request_close/shutdown never refunds live ownership. Only explicit host-control
flush clears native query Budget after the actual last physical owner and root
header are freed, or after unopened cancellation. Static service credit remains
charged, including after shutdown. Forgotten owners retain the envelope.

From the Rust workspace:

    cargo +nightly-2026-08-22 run --locked -p tidb-tiforth-memory --example native_owned_root

The example uses the public API and actual native ResourcePool. No production
server dependency/global allocator override, System substitution, fake host
counter, leaked service, Arrow route, global hook, SQL/E2E or RSS claim is added.
Fresh independent review/verification and all owner gates remain separate.
