# DeadPureScalarElimination beside a retained join invariant, selected leg

`build.omg` authors `Optimization::CopyPropagation` and
`Optimization::DeadPureScalarElimination` by exact name; `rollback.txt`
declares only `DeadPureScalarElimination`, so the request's
`OptimizationRollback` overlay subtracts the liveness rule before Terminal
production.

`main.omg` is the consumed literal join customer: `table[index]` owns a
runtime bound obligation, so the `joined` header retains an interval
invariant naming `probe` and `choice`. Both are semantic uses even though no
executable operand reads `probe`, so liveness keeps them while removing
`unused`, the dead comparison nothing consumes. The retained predicate is a
proof question the rewrite answers verbatim -- nothing is withheld, and the
compile report carries no `optimizer withheld` line for this fixture.

On the check leg the corpus runner compiles this fixture to its retained
Terminal artifact twice -- with and without the overlay -- and records
`module:differs`: subtracting the rule restores the module where `unused`
still occupies an operation, so the selected rule demonstrably rewrote the
module rather than withholding.

On the native leg the authored program exits `0`; the `bad` arm is a
`crash Trap` so a join that produced the wrong literal cannot pass.

`join_invariant_copy_transport_selected` shares this `main.omg` and declares
the other rule; `join_invariant_transport_control` selects both rules on a
program with no retained join invariant.
