# Interpreted Omega bootstrap experiment

This gate tests where Alpha tape production should resume after the selected
Gamma evaluator begins interpreting higher compiler sources.

The selected experiment keeps Epsilon execution interpreted and makes
`alpha_bootstrap` an ordinary Omega product target:

```text
Delta-written Epsilon evaluator + exact Epsilon-written Omega D
  -> interpreted Omega compiler D
  -> Omega C for alpha_bootstrap
  -> omega compiler Alpha tape
```

The gate requires the Omega product build to bind exactly one
`alpha_bootstrap::ProgramEntry`, requires Omega D to retain its Alpha tape
construction, and rejects any `EpsilonAlpha`/`epsilon_alpha_` backend residue in
the Delta-written Epsilon implementation. Removing that duplicate backend
and adding the first execution slice reduces the current Epsilon source from
9,460 lines / 468,672 bytes to 8,658 lines / 430,747 bytes.

The first executable slice runs complete checking, locates the fixed
`Main::main`, and executes an accepted empty entry sequence as exit zero with
empty stdout. Every nonempty entry remains explicitly `Unsupported`; that
staging outcome is not an Epsilon observation and cannot survive in the final
evaluator. The exact evaluator plus six-line driver compiles to a 507,153-byte
Gamma receipt, and the retained fixture executes to control byte `0x07`.

This is executable boundary evidence, not a completed interpreter edge.
Acceptance still requires execution of all Epsilon statements, expressions,
state transfers, traps, and Console observations, followed by exact composition
with D.
