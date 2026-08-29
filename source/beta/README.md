# `source/beta/` — Beta language and compiler implementations

This directory owns the Beta language and the compiler accepting Beta. Under
D11 the canonical compiler edge is implemented in Alpha and produces the
platform-independent Beta compiler tape. The existing
[`compiler/bc.beta`](compiler/bc.beta) self-host remains useful differential and
fixed-point evidence; reproduction does not make it the required predecessor
edge or prove compiler correctness.

```text
compiler/     bc source, artifact, Alpha cold start, and adjacent validation
reference/    optional executable reference meaning
test.sh       focused Beta language gate
```

## Current construction and migration

```text
bc-alpha.alpha --(Alpha seed + assembler)--> cold-start compiler
bc.beta        --(cold-start compiler)-----> initial bc tape
bc.beta        --(initial bc)--------------> persisted fixed-point beta_compiler_bytecode.tape
```

[`compiler/cold-start/`](compiler/cold-start/README.md) owns the lower-rooted
construction. It covers the exact pinned Beta surface, rebuilds `bc.beta`, and
reaches the accepted [`compiler/artifacts/beta_compiler_bytecode.tape`](compiler/artifacts/README.md)
without a Rust producer. The current tape is 40,693 bytes.

The Alpha-written `cold-start/bc-alpha.alpha` must become or construct the
complete canonical Beta compiler used by the direct chain. Any remaining
dependence on compiling `bc.beta` is migration work, not a permanent extra
self-host stage.

[`compiler/validation/`](compiler/validation/README.md) currently owns
source/artifact checking for `bc.beta`. Its bounded gates reconstruct artifact
framing and the exact maximal observation specified by
[`MAXIMAL_OBSERVATION.md`](compiler/validation/MAXIMAL_OBSERVATION.md). The
Alpha-owned checker is already constructed below `bc`. This work is not the
canonical Beta edge unless its general Alpha-target proof machinery is adapted
to the promoted Alpha-written compiler; source-specific remainder is deleted.

## Role in the lattice

The admitted `beta_compiler_bytecode.tape` consumes the Beta-written Gamma
compiler and emits `gamma_compiler_bytecode.tape`. It does not parse Delta.
The Alpha-owned derivation checker is a trust-floor service beside these
producer edges, not another compiler rung.

Run the migration and diagnostic gates directly with:

```sh
sh source/beta/test.sh
sh source/beta/compiler/cold-start/rebuild-artifact.sh --check
sh source/beta/compiler/cold-start/test.sh
sh source/beta/compiler/validation/admission/bc-artifact-structure.sh
sh source/beta/compiler/validation/admission/bc-block-control.sh
```

The active reduction and admission work is tracked in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
