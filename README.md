# logwise

![logo](https://github.com/drewcrawford/logwise/raw/main/art/logo.png)

A privacy-first structured observability facade for Rust. Zero dependencies,
`no_std`, no allocation, and no behavior at all until an application installs a
runtime.

```console
$ cargo tree -p logwise
logwise v0.7.0
```

That is the whole dependency tree, and a CI gate keeps it that way.

## Why logwise

Instrumentation has a cost problem and a trust problem, and most logging
crates solve neither.

**The cost problem.** Instrumenting a library normally taxes everyone who
compiles it, whether or not they ever read a log line: a dependency tree, an
allocator, formatting work for fields nobody asked for. The `logwise` facade
depends on nothing, builds without `std` or `alloc`, and checks a cached
per-call-site interest mask *before* evaluating any field expression. With no
runtime installed, every operation is a non-allocating no-op. With a runtime
installed, a field is evaluated only if some destination is actually
authorized to receive it — once, even when three sinks want it, and never when
none do.

**The trust problem.** Conventional logging treats privacy as a deployment
concern: whatever you log goes wherever your sinks go, and keeping user data
out of a remote crash reporter is a matter of discipline. logwise makes
privacy a per-field schema axis with three tiers — `support` (safe to leave
the machine), `local` (retained locally only), and `secret` (never retained at
all) — and gives sinks *capabilities*, not promises. A remote sink is handed a
projection that support-safe fields are copied into; local-only and secret
values are not withheld by convention, they are never materialized into the
view that remote code receives.

**The vocabulary problem.** `debug`-versus-`info` says nothing about why an
event exists. logwise separates *class* (operational, diagnostic, forensic,
performance, metric) from *severity*, separates throwaway printf-debugging
from durable events with stable names and schemas, and makes timing questions
precise: wall time, active poll time, and wake latency are different
measurements with different macros.

## Status

The 0.7 series is a substantial, intentionally breaking rewrite. The public
contract described here — the facade package boundary, the call-site macros,
the privacy model, and the dispatch ABI — is the new design. The previous
implementation lives on in `logwise_runtime` while it is ported onto that
contract, and its legacy macros (`info_sync!` and friends) remain available
from there during the migration. Applications now explicitly install a runtime
instead of receiving implicit logging behavior. The API may still change.

## Quick start

### Instrumenting a crate

A library depends on the facade alone. For temporary printf-style debugging,
use ordinary Rust formatting:

```rust
# let task_id = 42;
logwise::log!("spawned task {task_id}");
```

`log!` is a diagnostic, debug-severity, ad-hoc text event. It stays compiled
in ordinary optimized builds, inherits the active context, and is local-only —
formatting erases field boundaries, so its text can never be relabelled safe
for remote destinations. Its arguments are not evaluated unless a local
observer requests the call site.

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

Unlabelled fields default to local-only. Prefixing a field with `detail`
keeps its expression unevaluated until an observer explicitly asks for
expensive detail.

### Observing from an application

The application picks a runtime, installs it, and registers destinations
(application-side setup; the runtime is a separate package):

```text
use std::sync::Arc;
use logwise_runtime::{ConsoleSink, DetailLevel, Filter};

fn main() {
    let runtime = logwise_runtime::init().expect("install logwise runtime");
    runtime.add_local_sink(Arc::new(ConsoleSink), Filter::new(), DetailLevel::Full);

    logwise::event!("app.started", pid = support(std::process::id()));
}
```

Each registered view narrows what it receives by hierarchical domain, stable
event name, class, minimum severity, or context lineage — and its entry point
(`add_remote_sink`, `add_local_sink`, `add_ephemeral_sink`) fixes the privacy
tier it is even capable of observing.

## The workspace

| Package | Role |
|---|---|
| `logwise` | The zero-dependency, `no_std`, no-alloc facade. Everything a library needs. |
| `logwise_runtime` | The standard runtime: dispatch, context storage, clocks, filtering, projection, sinks, the flight recorder. Hosts the legacy 0.6 implementation while it is ported to the facade contract. |
| `logwise_runtime/logwise_runtime_proc` | Temporary legacy procedural macros; removed when the port completes. |
| `logwise_runtime_wasm` | The structured `logwise_v1` wasm host transport, without depending on any wasm binding crate. |
| `logwise_compat_log` | Optional bridge importing `log` records as quarantined local-only events. |
| `logwise_compat_tracing` | Optional `tracing` layer importing spans, events, and causality the same way. |
| `logwise_integration_tests` | Cross-package acceptance tests, kept above the facade so its own graph stays empty. |

The facade may depend on nothing; runtimes may depend on the facade; no
runtime may depend on an executor. `scripts/facade_boundary` enforces the
first rule in CI, along with a detached `no_std`, no-alloc consumer fixture
proving the default build needs neither `std` nor `alloc`. Optional `alloc`
and `std` features exist for convenience layers without changing the default
graph.

## Call sites

### Events

The short `event!` form shown above defaults to operational, info-severity
metadata. An unusual durable site spells its policy explicitly:

```rust
# let task_id = 42_u64;
logwise::event!(
    class: forensic,
    severity: debug,
    name: "some_executor.task.polled",
    task_id = support(task_id),
);
```

`forensic!` is shorthand for exactly that class/severity pair, and `counter!`
and `measurement!` emit metric-class observations with the same field
grammar. The supported privacy labels are `support`, `local`, and `secret`;
primitive values and `&str` convert automatically, and custom types pass
`ValueRef::debug(&value)` or `ValueRef::display(&value)`.

A domain override is a static value and is rarely needed — event names are
already hierarchical:

```rust
# let task_id = 42_u64;
const SCHEDULER: logwise::Domain = logwise::domain!("some_executor.scheduler");
logwise::event!(
    domain: SCHEDULER,
    name: "some_executor.task.queued",
    task_id = support(task_id),
);
```

### Compile-time removal

Static removal belongs to the crate containing the call site. Put its local
feature or target condition directly on the invocation; logwise transcribes it
as `#[cfg]` elimination, so values and schema strings are absent from the
binary:

```rust
# let task_id = 42_u64;
logwise::event!(
    #[cfg(any())] // for example: feature = "logwise-forensic" in this crate
    "some_executor.task.woken",
    task_id = support(task_id),
);
```

The boundary gate compiles a fixture to object code and greps it: excluded
metadata must be gone, and ordinary optimized builds must *not* strip the
instrumentation that was supposed to stay.

### Context that follows the task

Durable context belongs to the task, not the thread that happens to poll it.
An executor captures a parent when work is spawned, stores the child token in
the task, and enters it only around each poll:

```rust
let parent = logwise::context::capture();
let task = logwise::context::child(parent, "some_executor.task");

// Immediately around Future::poll:
let _entered = logwise::context::enter(task);
```

`ContextToken` is a fixed-size copyable value, cheap to store in every task.
The runtime stores its parent lineage and separate non-parent links
(`logwise::context::link`); the enter guard is deliberately not sendable and
restores the previous thread/worker-local token on drop. With no runtime
installed, capture/child/link/enter are harmless no-ops.

### Spans and timing

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
accepts the call site, and the runtime records elapsed monotonic time — and
any threshold violation — when the guard drops. The guard captures its
originating context, so completion is attributed to where the work began even
if the guard crosses threads or another context becomes current first.

## How dispatch stays cheap

Each static `Callsite` owns a generation-keyed `Interest` cache. Call-site
macros check that interest before constructing anything, then synchronously
pass a borrowed `EventRef` to the single installed dispatcher — no owned
record, no queue, no future. Interest distinguishes core/detail cost and
support-safe/local-only/secret privacy groups, so the check that guards field
evaluation is the same check that guards privacy tiers.

An application does not implement this ABI directly. Its chosen runtime
installs one dispatcher and mutates filters and sinks behind that stable
pointer, advancing its configuration generation whenever interest changes; a
call site whose cached generation is stale simply recomputes. The interest
mask at each call site is the union of every authorized view, so a field
needed by three sinks is still evaluated once, and a field needed by none is
not evaluated at all. Targets without pointer-width atomics deliberately
remain no-runtime no-ops.

## Privacy is a capability, not a configuration

The standard runtime registers destinations through three entry points —
remote-safe, retained-local, and explicitly trusted ephemeral. All implement
the same `EventSink` contract, but each receives a `ProjectedEvent` assembled
*after* authorization:

- remote views contain support-safe fields only and never receive ad-hoc
  text;
- retained-local views contain support-safe and local-only fields;
- trusted ephemeral views may additionally inspect secret fields during the
  synchronous call;
- secret fields are never copied into a runtime-owned retained event.

Each view also chooses core-only or full detail and may filter by domain,
event name, class, minimum severity, and context descendants.

TTL activation turns instrumentation up temporarily using the same selectors,
and answers honestly: `Enabled`, `UnavailableTarget`, `NotCompiled`, or
`UnknownSelector`. Activations retain a dynamic refinement bit in the
call-site cache, so expiry takes effect without waiting for unrelated
configuration changes. The runtime catalog reports every call site observed
in this build.

## Sinks and durability

Console and in-memory sinks consume projected events synchronously. Retaining
or I/O destinations copy the projection into an `OwnedProjectedEvent` with an
explicit per-string truncation limit; they still never see fields excluded by
their capability.

`AsyncSink` puts those owned events into a bounded queue and returns from the
log call immediately. Its default overflow policy drops the newest record and
counts it; overwrite-oldest is available for history-style queues. Accepted,
dropped, overwritten, truncated, and writer-error totals are all observable —
loss is accounted for, never silent. `flush()` yields a future barrier;
`flush_blocking()` and `emergency_drain()` are explicit blocking operations
for shutdown paths. Normal instrumentation never creates a future or waits
for I/O.

Runtime fan-out snapshots sink handles before invoking user code. Removing a
sink drops it after the configuration lock is released (sink destructors may
themselves log), recursive sink logging is counted and dropped, and
unwind-capable targets isolate a panicking sink so the remaining views still
run.

## Flight recorder

`FlightRecorder` is a retained-local, core-detail sink for recent structured
history — the "what just happened" buffer you consult after a bug report.
Capacity is fixed, writes are distributed across thread/worker shards, and
neither writers nor queries wait for a contended shard.

`read_since` returns globally sequence-ordered records plus the next
monotonic cursor, along with cumulative drop, overwrite, and truncation
totals, per-record omissions, and the number of shards that were busy. A
partial read does not advance its cursor, so a runner or platform mirror can
retry instead of silently losing the unavailable shard. `tail` provides the
newest bounded view.

Local reads contain support-safe and local-only fields. Remote reads clone
and project retained records down to support-safe fields and strip opaque
ad-hoc messages before the caller can serialize them. Secret fields are
rejected at recorder ingress even if the sink is invoked outside the standard
runtime.

## Foreign text ingress

First-party `logwise` events are the portable contract. Text intercepted from
anywhere else is best-effort input with no trusted privacy declaration, so it
all lands in one quarantine lane: the unstable-schema `foreign.text` event,
with local-only `origin` and `text` fields. Its kind is categorically
excluded from remote sinks; callers cannot relabel imported text
support-safe.

- `logwise_runtime::install_panic_hook()` chains a process-wide panic hook
  and returns the previous hook for explicit restoration.
- On Unix, `NativeFdCapture` can temporarily redirect stdout or stderr — a
  process-global operation with documented races, not a logical-context
  capture.
- The `foreign-nightly-rust-print` feature exposes an adapter over nightly
  Rust's unstable `internal_output_capture`, intended for test harnesses
  only. Nothing in the default facade or runtime claims to intercept
  arbitrary `println!` calls.
- `logwise_runtime_wasm::ingest_console` accepts text a host's console
  monkeypatch already intercepted, preserving origins such as
  `js.console.warn`.

The optional `logwise_compat_log` package installs a `log::Log`
implementation mapping levels, targets, messages, and key-values into
origin-marked local-only records. `logwise_compat_tracing::LogwiseLayer` maps
tracing span parentage to logwise context tokens, `follows_from` to links,
and events and fields into the same quarantined lane; compose it into an
existing subscriber or call its `install()`. Both bridges carry thread-local
reentrancy guards, so an outbound sink that itself logs cannot create
`log → logwise → log` recursion.

There is intentionally no built-in outbound bridge: flattening logwise into
either ecosystem would lose privacy projection, detail tiers, retention
policy, and part of the causal model.

## WebAssembly

wasm32 is a first-class target, not a port. `logwise_runtime_wasm` encodes
first-party events as allocation-free, versioned `logwise_v1` binary
envelopes preserving stable call-site metadata, typed fields and their
privacy/detail policy, context links, test/worker identity, and
sequence/drop/truncation accounting. Secret fields are defensively excluded.
Each call is a complete frame, so hosts mirror events incrementally instead
of waiting to query a guest that may be hung.

The `host-abi` feature imports `logwise_v1.emit(ptr, len)`. It is opt-in
because WebAssembly imports resolve at instantiation: embedders without the
ABI leave it disabled and receive `HostStatus::Unavailable`; embedders that
enable it must supply the import. The complete layout and a canonical host
vector live in `logwise_runtime_wasm/LOGWISE_V1.md`, and every test in the
workspace runs on both native and browser targets.

## Development

```console
scripts/check_all
```

runs the facade boundary gate, formatting, checks, clippy, tests, and docs
across native and wasm32, including the `no_std`/no-alloc fixture and the
dependency-tree check.

MSRV is Rust 1.95.0, edition 2024. Licensed under MIT OR Apache-2.0.
