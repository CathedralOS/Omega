# `source/beta/` — Beta language and compiler implementations

This directory owns the Beta language and the compiler accepting Beta. Under
D11 the canonical compiler edge is implemented in Alpha and produces the
platform-independent Beta compiler tape. No Beta self-host participates in or
shadows that edge.

```text
compiler/     canonical source, artifact, construction tests, and validation
reference/    optional executable reference meaning
```

## Canonical construction

```text
beta_compiler.alpha --(Alpha seed + assembler)--> beta_compiler_bytecode.tape
```

[`compiler/rebuild-artifact.sh`](compiler/rebuild-artifact.sh) owns the
lower-rooted construction. It rebuilds the accepted
[`compiler/beta_compiler_bytecode.tape`](compiler/beta_compiler_bytecode.tape)
directly, without a Rust producer or Beta self-host stage. The current tape is
20,977 bytes.

The Alpha-written [`compiler/beta_compiler.alpha`](compiler/beta_compiler.alpha)
is the complete canonical Beta compiler used by the direct chain.

[`compiler/validation/`](compiler/validation/README.md) retains the general
Alpha-tape structure checker and an Alpha-written exact encoding reconstructor
that target the canonical compiler. The
60k-line former self-host obligation tree, source/PC witnesses, and toy FOL
capability seam were deleted because none reconstructed the exact
Alpha-written source/tape proposition.

## Role in the lattice

The admitted `beta_compiler_bytecode.tape` consumes the Beta-written Gamma
compiler and emits `gamma_compiler_bytecode.tape`. It does not parse Delta.
The Alpha-owned derivation checker is a trust-floor service beside these
producer edges, not another compiler rung.

Run the construction and diagnostic gates directly with:

```sh
sh source/beta/compiler/rebuild-artifact.sh --check
sh source/beta/compiler/test.sh
sh source/beta/compiler/validation/admission/bc-artifact-structure.sh
sh source/beta/compiler/validation/admission/encoding/test.sh
```

The active reduction and admission work is tracked in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `compiler/` | The sole Alpha-written compiler accepting Beta, its exact Alpha tape, and adjacent edge validation. | Replace only atomically with the admitted immediate-predecessor compiler edge. |
| `reference/` | One untrusted executable interpretation of written Beta semantics used by focused differential gates. | Delete when a stronger semantic oracle fully subsumes every retained caller. |
