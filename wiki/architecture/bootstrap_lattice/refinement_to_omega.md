# Refinement to production Omega

[Lattice overview](bootstrap_lattice.md) · [Decisions](decisions.md) ·
[Proof kernel](proof_kernel.md)

Hosting does not establish correctness. Every compiler source is checked
directly against the exact Alpha tape that executes it:

```text
Beta-written Gamma compiler   ─▶ gamma_compiler_bytecode.tape
Gamma-written Delta compiler  ─▶ delta_compiler_bytecode.tape
Delta-written Epsilon compiler ─▶ epsilon_compiler_bytecode.tape
Epsilon-written Omega D       ─▶ omega0_compiler_bytecode.tape
Omega-written Omega C         ─▶ omega_compiler_bytecode.tape
```

Each arrow means “the tape refines the exact source semantics under the
reconstructed observation and resource profiles,” not merely “the compiler on
the left emitted these bytes.”

## Uniform artifact-side proof

Common Alpha targeting fixes the artifact half of every proposition:

- one tape decoder and instruction semantics;
- one memory, call, I/O, halt, trap, and exhaustion model;
- one resource-profile vocabulary; and
- one native-VM realization obligation per supported host.

The source half remains language-specific. A checker must reconstruct exact
source parsing, names, types, and operational semantics; it may not trust a
producer's AST or description of its own obligation.

High-level-to-Alpha distance may make certificates large. Checked intermediate
relations—typed blocks, CFGs, layouts, or local simulation lemmas—may compose the
proof. They are proof vocabulary under the same kernel, not additional
executables or permanent build dependencies.

## Required joins

Every accepted edge records:

- exact source closure and exact Alpha tape;
- canonical source semantics and Alpha semantics;
- exact input, observation, and resource profiles;
- reconstructed obligations and checked certificates;
- tape-format and seed-container identity where execution is requested; and
- transitively disclosed admissions.

Native stamping is a transparent container join. An optional Alpha-to-native
realization has a separate translation-validation proof and never replaces the
canonical tape proposition.

## Trusting trust

A malicious or defective predecessor may emit a bad tape, but it cannot select
the source semantics, observation profile, or reconstructed proposition. The
bad tape fails direct refinement. This is why diversified double compilation is
diagnostic rather than a required trust mechanism.

D39's `TerminalTraceV1` is the reusable observer at the Omega artifact edge.
It compares ordered external events and exact semantic values through normal
return, crash, declared successful external termination, and infinite maximal
execution. Compiler products compose sealed inputs, diagnostics, artifact
bytes, and product resource outcomes over that trace; native deployment composes
its separate formal-target-to-silicon admission afterward.

The first and self-hosted Omega compilers use different source closures, `D`
and `C`, so byte equality is neither expected nor required. Each independently
owes refinement to the full Omega language it implements.

The remaining work is tracked only in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
