# logwise

`logwise` is a zero-dependency, `no_std`, no-allocation-by-default facade for
structured observability.

The 0.7 rewrite keeps call sites tiny and moves filtering, clocks, context
storage, privacy projection, buffering, rendering, I/O, and agent integration
into runtime packages. With no runtime installed, facade operations are no-ops.

The facade contract is built around:

- static call-site metadata and stable event names;
- borrowed event values;
- cached interest checked before field evaluation;
- explicit privacy and detail policies per field;
- opaque context and span tokens;
- declarative ad-hoc and structured instrumentation macros.

The workspace currently contains:

- `logwise`: the foundational facade;
- `logwise_runtime`: the legacy implementation moved behind the new package
  boundary while it is ported to the facade contract;
- `logwise_runtime_proc`: temporary legacy procedural macros, which will be
  removed when the declarative facade macros land;
- `logwise_runtime_wasm`: the dependency-light reserved wasm host transport;
- `logwise_compat_log` and `logwise_compat_tracing`: optional,
  privacy-conservative foreign-ingress bridges;
- `logwise_integration_tests`: cross-package acceptance tests kept above the
  facade.

The redesign is intentionally breaking. The public 0.7 facade API is being
introduced in dependency order; applications will explicitly install a runtime
instead of receiving implicit logging behavior.

## Physical boundary

The facade declares no dependencies:

```console
$ cargo tree -p logwise
logwise v0.7.0
```

Its default build uses neither `std` nor `alloc`. Optional convenience layers
may enable `alloc`, or `std` (which includes `alloc`), without changing the
default foundational graph.

## Dispatch contract

Each static `Callsite` owns a generation-keyed `Interest` cache. Call-site
macros check that interest before constructing fields, then synchronously pass a
borrowed `EventRef` to the single installed dispatcher. Interest distinguishes
core/detail cost and support-safe/local-only/secret privacy groups.

An application does not install this ABI directly. Its chosen runtime installs
one dispatcher and mutates filters or sinks behind that stable pointer,
advancing its configuration generation whenever interest changes. With no
runtime installed, interest is empty and dispatch is a non-allocating no-op.

## Call-site macros

For temporary, portable printf-style debugging, use ordinary Rust formatting:

```rust
# let task_id = 42;
logwise::log!("spawned task {task_id}");
```

`log!` is a diagnostic, debug-severity ad-hoc text event. It remains compiled
in ordinary optimized builds, inherits the active context, and is local-only
because formatting erases field boundaries. Its arguments are not evaluated
unless a local observer requests the call site.

Durable instrumentation uses a stable event name and privacy-labelled fields:

```rust
# let task_id = 42_u64;
# let parent_id = 7_u64;
logwise::event!(
    "some_executor.task.spawned",
    task_id = support(task_id),
    parent_id = support(parent_id),
    detail task_name = local("worker"),
);
```

Unlabelled fields default to `LocalOnly`. The supported labels are `support`,
`local`, and `secret`; prefixing a field with `detail` keeps its expression
unevaluated until an observer explicitly asks for expensive detail. For custom
values, pass `ValueRef::debug(&value)` or `ValueRef::display(&value)`.

The short event form defaults to operational, info-severity event metadata. An
unusual durable site can spell its policy explicitly:

```rust
# let task_id = 42_u64;
logwise::event!(
    class: forensic,
    severity: debug,
    name: "some_executor.task.polled",
    task_id = support(task_id),
);
```

`span!`, `counter!`, and `measurement!` use the same field grammar. A domain
override is a static value and is rarely needed:

```rust
# let task_id = 42_u64;
const SCHEDULER: logwise::Domain = logwise::domain!("some_executor.scheduler");
logwise::event!(
    domain: SCHEDULER,
    name: "some_executor.task.queued",
    task_id = support(task_id),
);
```

## Context and timing

Durable context belongs to the task, not the thread that happens to poll it.
Capture a parent when work is spawned, store the child token in the task, and
enter it only around each poll:

```rust
let parent = logwise::context::capture();
let task = logwise::context::child(parent, "some_executor.task");

// Immediately around Future::poll:
let _entered = logwise::context::enter(task);
```

`ContextToken` is a fixed-size copyable value. The runtime stores its parent
lineage and separate non-parent links; the enter guard is deliberately not
sendable and restores the previous thread/worker-local token on drop. With no
runtime installed, capture/child/link/enter are harmless no-ops.

`span!` measures wall time from creation to guard drop. Performance-specific
helpers make the other timing questions explicit:

```rust
# use core::time::Duration;
let _wall = logwise::span!("some_executor.task.wall");
let _poll = logwise::active_span!("some_executor.task.poll");
let _wake = logwise::wake_latency_span!("some_executor.task.wake_latency");
let _warning = logwise::perfwarn!(
    threshold: Duration::from_millis(100),
    name: "some_executor.task.slow",
);
```

The facade never reads a clock. It starts a runtime span only after interest
accepts the call site, and the runtime records elapsed monotonic time and any
threshold violation when the guard drops. The guard captures its originating
context, so completion remains correctly attributed across later context
switches.

## Runtime views and privacy

The standard runtime registers destinations through three capability-specific
entry points: remote-safe, retained-local, and explicitly trusted ephemeral.
All implement the same `EventSink` contract, but they receive only a
`ProjectedEvent` assembled after authorization:

- remote views contain `SupportSafe` fields only and never receive ad-hoc text;
- retained-local views contain support-safe and local-only fields;
- trusted ephemeral views may additionally inspect secret fields during the
  synchronous call;
- secret fields are never copied into a runtime-owned retained event.

Each view chooses core-only or full detail and may filter by hierarchical
domain, stable event name, class, minimum severity, and context/descendants.
The call-site interest mask is the union of those authorized views, so a field
needed by three sinks is still evaluated once, while a field needed by none is
not evaluated at all.

TTL activation uses the same selectors and reports `Enabled`,
`UnavailableTarget`, `NotCompiled`, or `UnknownSelector`. Activations retain a
dynamic refinement bit in the call-site cache, so expiry takes effect without
waiting for unrelated configuration changes. The runtime catalog reports the
call sites observed in this build.

## Sinks and durability

Console and in-memory sinks consume projected events synchronously. Retaining
or I/O destinations copy that projection into an `OwnedProjectedEvent` with an
explicit per-string truncation limit; they still never see fields excluded by
their capability.

`AsyncSink` puts those owned events into a bounded runtime queue and returns
from the log call immediately. Its default overflow policy drops the newest
record and counts it; overwrite-oldest is available for history-style queues.
Accepted, dropped, overwritten, truncated, and writer-error totals are
observable. An outstanding durability barrier protects earlier accepted
records from later overwrite.

Creating `AsyncSink::flush()` yields a future barrier, while
`flush_blocking()` and `emergency_drain()` are explicit blocking operations.
The worker writes and flushes every accepted sequence through the barrier;
normal instrumentation never creates a future or waits for I/O.

Runtime fan-out snapshots sink handles before invoking user code. Removing a
sink drops it after the configuration lock is released, recursive sink logging
is counted and dropped, and unwind-capable targets isolate sink panics so the
remaining views still run. Panic-abort targets retain their platform's normal
abort semantics.

## Flight recorder

`FlightRecorder` is a retained-local, core-detail sink for recent structured
history. Capacity is fixed, writes are distributed across thread/worker shards,
and neither writers nor queries wait for a contended shard. Register it like
any other runtime sink (application-side setup):

```text
use std::sync::Arc;
use logwise_runtime::{DetailLevel, Filter, FlightRecorder};

let runtime = logwise_runtime::Runtime::new();
let recorder = Arc::new(FlightRecorder::new(1024, 256));
runtime.add_local_sink(
    recorder.clone(),
    Filter::new(),
    DetailLevel::Core,
);
```

`read_since` returns globally sequence-ordered records plus the next monotonic
cursor. It also returns cumulative drop, overwrite, and truncation totals,
per-record omissions, and the number of shards that were busy. A partial read
does not advance its cursor, so a runner or platform mirror can retry instead
of silently losing the unavailable shard. `tail` provides the newest bounded
view.

Local reads contain support-safe and local-only fields. Remote reads clone and
project retained records down to support-safe fields and remove opaque ad-hoc
messages before the caller can serialize them. Secret fields are rejected at
recorder ingress even if the sink is invoked outside the standard runtime.
Rendering is deferred until a retrieved `FlightRecord` is displayed.

## Foreign text ingress

First-party `logwise` events are the portable contract. Text intercepted from
Rust printing, native file descriptors, panic hooks, or JavaScript consoles is
best-effort input. Every adapter emits the unstable-schema `foreign.text` event
with local-only `origin` and `text` fields. Its ad-hoc kind is categorically
excluded from remote sinks; callers cannot relabel imported text support-safe.

`logwise_runtime::install_panic_hook()` chains a process-wide panic hook and
returns the previous hook for explicit restoration. On Unix,
`NativeFdCapture` can temporarily redirect stdout or stderr, but this is a
process-global operation: it races parallel writers, has no logical-context
boundary, and does not match libtest's thread-local Rust print capture.

The `foreign-nightly-rust-print` runtime feature exposes the separate
`capture_nightly_rust_print` adapter. It requires nightly Rust's unstable
`internal_output_capture` facility and is intended for test harnesses only.
Nothing in the default facade or runtime claims to intercept arbitrary
`println!` or `eprintln!` calls.

`logwise_runtime_wasm::ingest_console` accepts text already intercepted by a
host's console monkeypatch and preserves origins such as `js.console.warn`.
The host remains responsible for the monkeypatch. This legacy ingress does not
replace the structured `logwise_v1` event transport, and `console.trace` is not
treated as an ordinary trace-level line because browsers assign it stack-trace
semantics.

The optional `logwise_compat_log` package installs a `log::Log` implementation.
It maps levels, targets, messages, module/location data, and key-values into
origin-marked local-only records. `logwise_compat_tracing::LogwiseLayer` maps
tracing span parentage to logwise context tokens, `follows_from` relationships
to links, and events, levels, targets, IDs, and fields into the same quarantined
lane. It can be composed into an existing subscriber or installed with its
convenience `install()` function.

Both bridges use thread-local reentrancy guards, so an application-side lossy
outbound sink cannot create `log -> logwise -> log` or
`tracing -> logwise -> tracing` recursion. Imported dynamic fields have no
trusted privacy declaration and therefore cannot be upgraded from `LocalOnly`.
There is intentionally no built-in outbound bridge: flattening logwise into
either ecosystem loses privacy projection, detail tiers, retention policy, and
part of the causal model.

## Structured wasm transport

`logwise_runtime_wasm` encodes first-party events as allocation-free,
versioned `logwise_v1` binary envelopes. The wire preserves stable call-site
metadata, typed fields and their privacy/detail policy, context links,
test/worker identity, and sequence/drop/truncation/omission accounting. Secret
fields are defensively excluded. Each call is a complete frame, so hosts can
mirror it incrementally instead of waiting to query a guest that may be hung.

The `host-abi` feature imports `logwise_v1.emit(ptr, len)`. It is opt-in because
WebAssembly imports are resolved at instantiation: embedders without the ABI
leave the feature disabled and receive `HostStatus::Unavailable`; embedders
that enable it must supply the import. Accepted, unavailable, version-mismatch,
dropped, and host-error results are distinct. The complete layout and canonical
host vector live in `logwise_runtime_wasm/LOGWISE_V1.md`.

Static removal belongs to the crate containing the call site. Put its local
feature or target condition directly on the invocation; logwise transcribes it
as `#[cfg]` elimination, so values and schema strings are absent:

```rust
# let task_id = 42_u64;
logwise::event!(
    #[cfg(any())] // for example: feature = "logwise-forensic" in this crate
    "some_executor.task.woken",
    task_id = support(task_id),
);
```

Run the complete workspace gate with:

```console
scripts/check_all
```

The gate checks the dependency tree and compiles a detached `no_std`, no-alloc
consumer fixture in addition to the native and wasm workspace checks.
