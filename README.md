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

Run the complete workspace gate with:

```console
scripts/check_all
```

The gate checks the dependency tree and compiles a detached `no_std`, no-alloc
consumer fixture in addition to the native and wasm workspace checks.
