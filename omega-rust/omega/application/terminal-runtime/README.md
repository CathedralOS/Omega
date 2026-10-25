# Hosted Terminal execution

Start at [execution.rs](src/execution.rs). This application owner consumes an
already-produced canonical Terminal artifact, exact runtime provisions and an
explicit host request. It admits the requested target and bindings, establishes
private entry storage, installs host adapters and executes through the Psi
interpreter. Fuel replenishment resumes that same instance.

The language engine remains in `psi/interpreter/terminal-interpreter`.
Target-aware ABI/locator checks, finite input replay, buffered output, virtual
clocks and isolated-domain exit delegation belong here, not in a compiler.
The CLI, corpus and embedding consumers call this owner directly; there is no
`compiler::runtime` compatibility route.

This extraction changes ownership, not runtime semantics. It does not complete
**TERMINAL-RUNTIME-DRIVERS**: the retained provision type still comes from
`compilation-products`, whose dependency graph is not source-free. The common
loaded/borrowed Terminal view, source-free provision contract and production
artifact-execution command remain required. Moving host code must not put target
knowledge into Psi or replace missing execution with native fallback.
