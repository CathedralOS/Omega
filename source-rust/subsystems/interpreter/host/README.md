# Hosted Terminal execution

Start at [execution.rs](src/execution.rs). This package runs an
already-produced canonical Terminal artifact with exact runtime provisions and
an explicit host request. It admits the requested target and bindings, sets up
private entry storage, installs host adapters and runs it on the Psi
interpreter. Replenishing work resumes the same instance.

The portable `execute` and `execute_sliced` entries run without a native image. The [retained-image host](retained-image/execution.rs) also accepts the completed image for programs that observe linked code layout. It checks the supplied image against the target and the canonical Psi manifest before reading the root machine descriptors. Build’s registered test runner uses this entry for comparisons and reports inspection failures as execution errors.

[Clock bindings](clock/bindings.rs) and [filesystem bindings](filesystem/bindings.rs)
admit the calls this host can service. The [native worker](native-worker/src/lib.rs)
owns the operating-system clock calls and filesystem handles those bindings use.

The native sponsor installs only completed foreign-adapter images. Stage 15 issues
a `NativeForeignAdapterImage` after it emits a shared container from a closed
adapter object and hashes the emitted bytes. The sponsor keeps these concrete
products in its roster. It matches the requested target and ABI, normalized
locator, application commitment and image digest before passing the product's
export and bytes to the worker. The worker checks the digest again before
loading it. This publication witness is the sponsor’s production dependency on
image emission. The worker’s emitter dependency builds fixtures for its tests.
[Publisher](../../compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/entries.rs)
and [sponsor](native-sponsor/sponsor.rs).

The language engine lives in `psi/interpreter`. Its low-level embedding API
accepts explicit host environments. This crate supplies target-aware host
bindings, private entry storage, finite input replay, buffered output, clocks
and isolated-domain exit delegation. The public portable request accepts
the source-free execution provisions published for its canonical program.
