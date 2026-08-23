# `bootstrap/rungs/gamma/` — safe definitional computation

Gamma is the pure functional rung above Beta. It supplies algebraic data,
pattern matching, recursion, fuel-bounded reference evaluation, and a small
static type system. It is suitable for parsers, validators, interpreters, and an
implementation of the cross-cutting proof kernel.

The reference evaluator implements proper tail calls and uses compact internal
representations for dense integers and ordinary `Cons` cells. These are bounded
execution properties of the canonical interpreter, not extra Gamma syntax;
printed values and pattern matching retain the language definition in
[`LANGUAGE.md`](LANGUAGE.md).

The canonical implementation path is Rust-free:

```text
interp.beta / typeck.beta --bc.beta--> Alpha assembly --assembler--> tape --seed-->
    canonical interpreter / type checker consuming Gamma source
```

Principal artifacts:

- `interp.beta` — canonical Gamma reference interpreter, written in Beta;
- `typeck.beta` — monomorphic Gamma type checker, written in Beta;
- `bootstrap/assurance/proof-kernel/implementations/gamma/` — independent
  proof-kernel implementations hosted by Gamma and owned by assurance;
- `canonical-bytes/` — reusable typed byte-cursor primitives;
- `terminal-codec-primitives/` — reusable typed scalar, identity, type, integer-
  value, UTF-8, and structural-leaf grammar fragments. It deliberately owns no
  fixed terminal-format header or complete live codec;

Run the principal gates from the repository root:

```sh
sh bootstrap/rungs/gamma/test-interp.sh
sh bootstrap/rungs/gamma/test-typeck.sh
sh bootstrap/assurance/proof-kernel/gates/gamma-checker.sh
sh bootstrap/rungs/gamma/test-canonical-bytes.sh
sh bootstrap/rungs/gamma/test-terminal-codec-primitives.sh
```

The former terminal-ledger spike was retired after its format-18/vocabulary-20
decoder became stale. Its architectural result is retained in the terminal-Psi
documentation and Git history (`a5cfd83cc`); the live production semantic tables
now own its useful closed-row decomposition. Do not recreate format-specific
verification permutations under the Gamma language owner.

## Ownership classification

Only `LANGUAGE.md`, `interp.beta`, and `typeck.beta` define Gamma's accepted
surface, canonical evaluation, and static checking. The independent Python
evaluator and the focused gates are conformance tools; agreement with them does
not override `interp.beta`.

`canonical-bytes/` and `terminal-codec-primitives/` are reusable programs in
Gamma. Being written in and evaluated by Gamma does not make a consumer part of
the language or its meaning. Artifact-specific obligation reconstruction belongs
under assurance, not under the Gamma rung.

## Parked imperative Gamma

`gamma.alpha`, `gamma_x64_windows.exe`, `build.sh`, `rebuild.sh`, and the root
`examples/` directory implement the older compiler-first language with variables
`a`–`j`, mutation, `if`/`while`, and decimal I/O. They remain compatibility and
differential-testing artifacts only. They do not define canonical Gamma and
must not grow into a second meaning path.

The parked files remain co-located for compatibility only. Their classification,
not their host-language suffix or directory proximity, determines their
architectural role. `compiler/gamma` is a temporary compatibility symlink to
this directory.

See [LANGUAGE.md](LANGUAGE.md) for the canonical surface and
[`rungs/gamma.md`](../../../wiki/architecture/bootstrap_lattice/rungs/gamma.md) for
Gamma's architectural role.
