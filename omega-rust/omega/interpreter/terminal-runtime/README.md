# Hosted Terminal execution

Start at [execution.rs](src/execution.rs). This package runs an
already-produced canonical Terminal artifact with exact runtime provisions and
an explicit host request. It admits the requested target and bindings, sets up
private entry storage, installs host adapters and runs it on the Psi
interpreter. Replenishing work resumes the same instance.

If a native image supplied for code-layout observations fails inspection,
that is an execution error, not a missing host capability.
So the CLI fails `--both` rather than skip comparison with a successful
native exit. A truly missing host provision can still make comparison unavailable.

The language engine stays in `psi/interpreter`.
Target-aware ABI/locator checks, finite input replay, buffered output,
virtual clocks and isolated-domain exit delegation belong here, not in a
compiler. The CLI and embedders call this package directly; there is
no `compiler::runtime` compatibility route.

This move changed ownership, not runtime semantics. It does not
complete **TERMINAL-RUNTIME-DRIVERS**: provisions come straight from their
source-free `native-artifact` owner, not through the source-aware `products`
re-export. The common loaded/borrowed Terminal view and production
artifact-execution command are still needed. Moving host code must not put
target knowledge into Psi or replace missing execution with native fallback.
