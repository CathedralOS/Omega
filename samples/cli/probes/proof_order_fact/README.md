# proof_order_fact

First sample that is pure proof artifact: `requires i < j
ensures i != j {}` — the empty body discharges from the
requirement via built-in order rules and emits no runtime
code. `machine Main::main()` parameterless bound entry.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
