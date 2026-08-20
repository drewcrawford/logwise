# Rewrite acceptance matrix

`scripts/check_all` is the release gate. It runs the physical facade checks,
then check, Clippy, tests, and documentation for both native and browser-wasm
targets. The architectural claims are pinned to these automated cases:

| Claim | Automated coverage |
|---|---|
| Zero normal/build dependencies, no-std/no-alloc, excluded metadata absent | `scripts/facade_boundary`, `fixtures/no_std_no_alloc` |
| No-runtime allocation/evaluation and generation-cached interest | `tests/no_runtime.rs`, `tests/dispatch.rs` |
| Core/detail selective materialization and disabled spans | `logwise_integration_tests/src/lib.rs`, `logwise_runtime/src/facade_runtime.rs` |
| Local/remote/ephemeral privacy projection and foreign-text quarantine | `privacy_projection.rs`, `foreign_ingress.rs` |
| Migrating task context, poll restoration, lifecycle, descendant TTL | `executor_context.rs` |
| Bounded cursors, overwrites/drops, truncation, panic/reentrancy | `flight_recorder.rs`, `runtime_sinks.rs` |
| Wasm worker/test identity, incremental history, transport loss | `wasm_wire.rs`, `logwise_runtime_wasm/tests/golden.rs` |

Cross-package behavior belongs in `logwise_integration_tests`; the facade's own
tests and fixtures deliberately use no runtime or executor dependency.
