# `source/beta/` — Beta language and compiler implementations

This directory owns the Beta language and the compiler accepting Beta. Under
D11 the canonical compiler edge is implemented in Alpha and produces the
platform-independent Beta compiler tape. The existing
[`compiler/bc.beta`](compiler/bc.beta) self-host remains useful differential and
self-host evidence; reproduction does not make it the required predecessor
edge or prove compiler correctness.

```text
compiler/     canonical source, artifact, construction tests, and validation
reference/    optional executable reference meaning
```

## Canonical construction

```text
beta_compiler.alpha --(Alpha seed + assembler)--> beta_compiler_bytecode.tape
```

[`compiler/cold-start/`](compiler/cold-start/README.md) owns the lower-rooted
construction. It rebuilds the accepted
[`compiler/artifacts/beta_compiler_bytecode.tape`](compiler/artifacts/README.md)
directly, without a Rust producer or Beta self-host stage. The current tape is
20,977 bytes.

The Alpha-written [`compiler/beta_compiler.alpha`](compiler/beta_compiler.alpha)
is the complete canonical Beta compiler used by the direct chain. Remaining
`bc.beta` executions are bounded differential diagnostics, never construction.

[`compiler/validation/`](compiler/validation/README.md) retains the general
Alpha-tape structure checker, ordinary-FOL simulation seams, and bounded
differential stress tools that can target the promoted compiler. The 60k-line
exact-`bc.beta` obligation tree and source/PC witnesses were deleted because
their proposition cannot admit an Alpha-written source.

## Role in the lattice

The admitted `beta_compiler_bytecode.tape` consumes the Beta-written Gamma
compiler and emits `gamma_compiler_bytecode.tape`. It does not parse Delta.
The Alpha-owned derivation checker is a trust-floor service beside these
producer edges, not another compiler rung.

Run the construction and diagnostic gates directly with:

```sh
sh source/beta/compiler/cold-start/rebuild-artifact.sh --check
sh source/beta/compiler/cold-start/test.sh
sh source/beta/compiler/validation/admission/bc-artifact-structure.sh
```

The active reduction and admission work is tracked in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
