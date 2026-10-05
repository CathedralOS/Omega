# CopyPropagation on a retained join invariant, selected leg

`build.omg` authors `Optimization::CopyPropagation` and
`Optimization::DeadPureScalarElimination` by exact name; `rollback.txt`
declares only `CopyPropagation`, so the request's `OptimizationRollback`
overlay subtracts the copy rule before Terminal production.

`main.omg` is the consumed literal join customer: `table[index]` owns a
runtime bound obligation, so the `joined` header retains an interval
invariant over `choice` (the literal-borne parameter) and `probe` (a copy
parameter every edge binds to `index`). The selected copy rule resolves
`probe` to `index`, a source in the join header's telescope, drops the
parameter and its edge arguments, and rewrites the retained predicate with
the same substitution -- `probe <= 7` becomes `index <= 7` -- while the
independent verifier transports the existing certificates through that same
copy relation. Nothing is withheld: the compile report carries no
`optimizer withheld` line for this fixture.

On the check leg the corpus runner compiles this fixture to its retained
Terminal artifact twice -- with and without the overlay -- and records:

- `rollback:requested=[CopyPropagation];applied=[CopyPropagation];effective=[DeadPureScalarElimination]`;
- `module:differs` -- the vacuity guard: subtracting the rule restores the
  module where `probe` still occupies the join's parameter table and the
  invariant still names it, so the selected rule demonstrably rewrote the
  module rather than withholding.

On the native leg the authored program exits `0`: `choose(1, 2)` reads
`table[1]`, picks the `true` arm's literal `10` through the consumed join,
and `main` compares `decided == 10` before reaching `done`. The `bad` arm is
a `crash Trap` so a join that produced the wrong literal cannot pass.

`join_invariant_dead_scalar_transport_selected` shares this `main.omg` and
declares the other rule; `join_invariant_transport_control` selects both
rules on a program with no retained join invariant.
