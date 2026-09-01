# `source/gamma/` — Gamma language and compiler implementations

This directory owns the Gamma language and the compiler accepting Gamma. Under
D11 the canonical compiler edge is implemented in Beta and produces the
platform-independent Gamma compiler tape. No Gamma self-host participates in or
shadows that edge.

`compiler/` contains only the canonical source, artifact, code table, and owner
documentation. Construction tools live under `tools/bootstrap/gamma/`; tests
and executable references live under `tests/gamma/`.

## Canonical construction

```text
gamma_compiler.beta --(direct Beta assembler tape)--> gamma_compiler_bytecode.tape
```

`tools/bootstrap/gamma/rebuild-artifact.sh` owns the direct construction. It rebuilds the accepted
[`compiler/gamma_compiler_bytecode.tape`](compiler/gamma_compiler_bytecode.tape)
directly, without a Rust producer or Gamma self-host stage. The current tape is
27,087 bytes.

The Beta-written [`compiler/gamma_compiler.beta`](compiler/gamma_compiler.beta)
is the complete canonical Gamma compiler used by the direct chain.

`tests/gamma/compiler/validation/` retains the general
Alpha-tape structure checker that targets the canonical compiler. The
status-only encoding reconstructor and the
60k-line former self-host obligation tree, source/PC witnesses, and toy FOL
capability seam were deleted because none reconstructed the exact
Beta-written source/tape proposition.

## Role in the bootstrap chain

The admitted `gamma_compiler_bytecode.tape` consumes the Gamma-written Delta
compiler and emits `delta_compiler_bytecode.tape`. It does not parse Delta.
The Alpha-owned derivation checker is a trust-floor service beside these
producer edges, not another compiler rung.

Run the construction and diagnostic gates directly with:

```sh
sh tools/bootstrap/gamma/rebuild-artifact.sh --check
sh tests/gamma/compiler/test.sh
sh tests/gamma/compiler/validation/admission/gc-artifact-structure.sh
```

The active reduction and admission work is tracked in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `compiler/` | The sole Beta-written compiler accepting Gamma, its exact Alpha tape, and closed outcome table. | Replace only atomically with the admitted immediate-predecessor compiler edge. |
| `LANGUAGE.md`, `SEMANTICS.md`, `CALLING_CONVENTION.md` | The accepted Gamma surface, execution relation, and compiler/Alpha frame contract. | Replace only atomically with a ruled Gamma revision and synchronized compiler tests. |
