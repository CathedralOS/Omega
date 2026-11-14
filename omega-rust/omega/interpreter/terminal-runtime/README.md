# Hosted Terminal execution

Start at [execution.rs](src/execution.rs). This application owner consumes an
already-produced canonical Terminal artifact, exact runtime provisions and an
explicit host request. It admits the requested target and bindings, establishes
private entry storage, installs host adapters and executes through the Psi
interpreter. Work replenishment resumes that same instance.

When a completed native image is supplied for code-layout observations, failed
inspection is an execution error, not an unavailable host capability. The CLI
therefore fails `--both` instead of declining comparison with a successful native
exit. A genuinely missing host provision can still make comparison unavailable.

The language engine remains in `psi/interpreter/terminal-interpreter`.
Target-aware ABI/locator checks, finite input replay, buffered output, virtual
clocks and isolated-domain exit delegation belong here, not in a compiler.
The CLI, corpus and embedding consumers call this owner directly; there is no
`compiler::runtime` compatibility route.

This extraction changes ownership, not runtime semantics. It does not complete
**TERMINAL-RUNTIME-DRIVERS**: provisions come directly from their source-free
`native-artifact` owner, not through the source-aware `compilation-products`
re-export. The common loaded/borrowed Terminal view and production
artifact-execution command remain required. Moving host code must not put target
knowledge into Psi or replace missing execution with native fallback.
