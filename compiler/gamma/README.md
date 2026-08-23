# `compiler/gamma/` — safe definitional computation

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
- `terminal-codec-primitives/` — exact terminal-Psi codec primitives;
- `terminal-ledger-spike/` — bounded canonical semantic-ledger feasibility work.

Run the principal gates from the repository root:

```sh
sh compiler/gamma/test-interp.sh
sh compiler/gamma/test-typeck.sh
sh bootstrap/assurance/proof-kernel/gates/gamma-checker.sh
sh compiler/gamma/test-canonical-bytes.sh
sh compiler/gamma/test-terminal-codec-primitives.sh
sh compiler/gamma/test-terminal-ledger-spike.sh
```

## Parked imperative Gamma

`gamma.alpha`, `gamma_x64_windows.exe`, `build.sh`, `rebuild.sh`, and the root
`examples/` directory implement the older compiler-first language with variables
`a`–`j`, mutation, `if`/`while`, and decimal I/O. They remain compatibility and
differential-testing artifacts only. They do not define canonical Gamma and
must not grow into a second meaning path.

See [LANGUAGE.md](LANGUAGE.md) for the canonical surface and
[`rungs/gamma.md`](../../wiki/architecture/bootstrap_lattice/rungs/gamma.md) for
Gamma's architectural role.
