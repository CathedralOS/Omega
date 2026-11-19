# Hosted Terminal execution

Start at [execution.rs](src/execution.rs). This package runs an
already-produced canonical Terminal artifact with exact runtime provisions and
an explicit host request. It admits the requested target and bindings, sets up
private entry storage, installs host adapters and runs it on the Psi
interpreter. Replenishing work resumes the same instance.

The portable `execute` and `execute_sliced` entrances run without a native
image. A program that observes a machine's linked code layout uses Build's
[published program execution](../../compiler/build/published_terminal.rs). Full and sliced execution both take the retained native artifact. Build checks its target and
canonical Terminal identity, replays the final image against its object, and
reads root machine descriptors from the linked sections. A failed inspection
is an execution error. The CLI reports it when running `--both`.

[Clock bindings](clock/bindings.rs) and [filesystem bindings](filesystem/bindings.rs)
admit the calls this host can service. The [native worker](native-worker/src/lib.rs)
owns the operating-system clock calls and filesystem handles those bindings use.

The language engine lives in `psi/interpreter`. Its low-level embedding API
accepts explicit host environments. This crate supplies target-aware host
bindings, private entry storage, finite input replay, buffered output, clocks
and isolated-domain exit delegation. The public portable request accepts
the source-free execution provisions published for its canonical program.
