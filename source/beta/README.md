# `source/beta/` — the self-hosting Beta compiler

This directory owns the Beta language and the compiler written in Beta:
[`compiler/bc.beta`](compiler/bc.beta). The compiler reproduces its persisted
platform-independent tape byte-for-byte. That fixed point establishes
deterministic self-reproduction, not compiler correctness.

```text
compiler/     bc source, artifact, Alpha cold start, and adjacent validation
reference/    optional executable reference meaning
test.sh       focused Beta language gate
```

## Construction

```text
bc-alpha.alpha --(Alpha seed + assembler)--> cold-start compiler
bc.beta        --(cold-start compiler)-----> initial bc tape
bc.beta        --(initial bc)--------------> persisted fixed-point beta_compiler_bytecode.tape
```

[`compiler/cold-start/`](compiler/cold-start/README.md) owns the lower-rooted
construction. It covers the exact pinned Beta surface, rebuilds `bc.beta`, and
reaches the accepted [`compiler/artifacts/beta_compiler_bytecode.tape`](compiler/artifacts/README.md)
without a Rust producer. The current tape is 40,693 bytes.

[`compiler/validation/`](compiler/validation/README.md) owns source/artifact
checking for this compiler. Its bounded default gates reconstruct artifact
framing and the exact maximal observation specified by
[`MAXIMAL_OBSERVATION.md`](compiler/validation/MAXIMAL_OBSERVATION.md). The
Alpha-owned checker is already constructed below `bc`; complete admission is
still open because its calculus lacks the guarded Alpha/Beta simulation rule
needed to accept that reconstructed proposition.

## Role in the lattice

The accepted `beta_compiler_bytecode.tape` builds Gamma's canonical evaluator and type checker.
Gamma then evaluates Delta through Delta's declared meaning route. The
Alpha-owned derivation checker is a trust-floor service beside these producer
edges, not another compiler rung.

Run the independently invocable gates with:

```sh
sh source/beta/test.sh
sh source/beta/compiler/cold-start/rebuild-artifact.sh --check
sh source/beta/compiler/cold-start/test.sh
sh source/beta/compiler/validation/admission/bc-artifact-structure.sh
sh source/beta/compiler/validation/admission/bc-block-control.sh
```

The active reduction and admission work is tracked in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
