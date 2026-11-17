# Build-time artifact execution

Requested source computations use the ordinary lowered-form checker and Psi
execution semantics. This crate prepares supported invocation requests, executes
their admitted artifacts and returns owned result snapshots with exact input and
resource accounting. It is not an alternate source interpreter. Omega owns Build
configuration, package acquisition and granted host effects.

## Owners and handoffs

| Responsibility | Entry point |
| --- | --- |
| Source evaluation requests and their actual callers | [source_jobs.rs](src/source_jobs.rs) |
| Execute canonical scalar applications | [scalar_applications.rs](src/scalar_applications.rs) |
| Transport arguments and results | [terminal_values.rs](src/terminal_values.rs) |
| Retain source and authority metadata | [source_frontier.rs](src/source_frontier.rs) |
| Build product requests and selected providers | [product.rs](src/product.rs) |

A prepared invocation binds the actual caller, selected declaration, arguments,
destination, source authority and accepted program. Execution cannot replace the
arguments after admission. A receipt records the exact artifact, target,
observations and work; it cannot admit unrelated code or incompatible bytes.

## Evaluation dependencies

A pending computation is a request, not a value. Preserve its selected operation,
binder substitution, destination and target/provider dependencies until the
inputs are available and the invocation is admitted. Its source position does
not license a separate interpretation of its initializer or operator token.

The compiler driver owns readiness and request ordering. A static argument that
uses a computed constant must retain that dependency. A fixed pass order cannot
substitute a guessed value or make two source contexts use different arithmetic.
Report genuine dependency cycles; an unsupported route names the unfinished
request rather than claiming its mathematics is false.

Use the ordinary checking, execution and materialization contracts. Do not patch
an ordering gap with a syntax predicate prover or a later stage calling backward
into an earlier producer. This requires explicit dependencies, not a universal
asynchronous scheduler.

An activation from a retained base answers the requests of the declarations it
keeps. Other sources remain with the base. An available result stays tied to its
exact application and admission; a missing result is not supplied by an
interpreter default or an implicit cross-build cache.

## Admission and implementation limits

[Semantic evaluation](../../../../wiki/spec/language/evaluation.md) owns pure
invocation admission, target semantics, snapshots and logical accounting.
[Build execution](../../../../wiki/spec/build/execution.md) separately owns host
grants and staged inputs. Neither route turns runtime-only assumptions or ambient
host state into a constant or proof.

`EARLY-TERMINAL-JOBS` in [TASKS.md](../../../../TASKS.md) tracks unfinished routes.
A request without a supported producer refuses explicitly. An entry point or
this README is not evidence that every constant, layout or access-policy form
executes.

A dependency repair exercises a checked computation used as a constant and a
static argument, with a downstream plan or program consuming the result. Retain
operator selection, source authority, carrier identity and target dependencies.
Include an unavailable dependency or genuine cycle, and an invalid predicate
that reaches its intended obligation. Repair the owning request path and its
consumer, not a second interpreter for the example.
