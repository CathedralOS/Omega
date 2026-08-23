# `bootstrap/rungs/gamma/` — safe definitional computation

Gamma is the pure functional rung above Beta. It supplies algebraic data,
pattern matching, recursion, fuel-bounded reference evaluation, and a small
static type system. It is suitable for parsers, validators, interpreters, and an
implementation of the cross-cutting proof kernel.

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
- `terminal-codec-primitives/` — exact primitives for the spike's frozen
  terminal format 18/vocabulary 20 slice;
- `terminal-ledger-spike/` — bounded historical semantic-ledger feasibility
  work; it is an artifact-assurance experiment, not Gamma language meaning.

Run the principal gates from the repository root:

```sh
sh bootstrap/rungs/gamma/test-interp.sh
sh bootstrap/rungs/gamma/test-typeck.sh
sh bootstrap/assurance/proof-kernel/gates/gamma-checker.sh
sh bootstrap/rungs/gamma/test-canonical-bytes.sh
sh bootstrap/rungs/gamma/test-terminal-codec-primitives.sh
```

The standalone ledger-spike gate is not currently a passing principal gate. Its
frozen format-18/vocabulary-20 decoder correctly rejects the live product's
format-22/vocabulary-25 fixtures. Rebase or retire the experiment before adding
it back to the default lattice suite; do not widen Gamma meaning to make it pass.

## Ownership classification

Only `LANGUAGE.md`, `interp.beta`, and `typeck.beta` define Gamma's accepted
surface, canonical evaluation, and static checking. The independent Python
evaluator and the focused gates are conformance tools; agreement with them does
not override `interp.beta`.

`canonical-bytes/` is a reusable program in Gamma. `terminal-codec-primitives/`
and `terminal-ledger-spike/` are frozen bounded feasibility artifacts for
artifact-specific terminal-Psi obligation reconstruction. Being written in and
evaluated by Gamma does not make that spike part of the language or its meaning.

## Parked imperative Gamma

`gamma.alpha`, `gamma_x64_windows.exe`, `build.sh`, `rebuild.sh`, and the root
`examples/` directory implement the older compiler-first language with variables
`a`–`j`, mutation, `if`/`while`, and decimal I/O. They remain compatibility and
differential-testing artifacts only. They do not define canonical Gamma and
must not grow into a second meaning path.

The parked files and the ledger spike remain co-located in this checkpoint only
to make the ownership move behavior-neutral. Their classification, not their
host-language suffix or directory proximity, determines their architectural
role. `compiler/gamma` is a temporary compatibility symlink to this directory.

See [LANGUAGE.md](LANGUAGE.md) for the canonical surface and
[`rungs/gamma.md`](../../../wiki/architecture/bootstrap_lattice/rungs/gamma.md) for
Gamma's architectural role.
