# Gamma evaluator

This owner contains trusted Beta source for the Gamma evaluator. The admitted
Beta compiler translates it to Alpha tape, making this the Beta-to-Gamma
compiler edge rather than optional reconstruction tooling.

The current slice implements exact-ended request loading, source-envelope
validation, one unary entry function, `if`, `let`, checked integer primitives,
all five byte primitives, total equality over its value universe,
`Complete`/`Reject`, immutable raw/hex/single/concat/slice nodes, linear local
lookup, bounded arena/stack collision checks, and terminal status classes. It
does not yet implement declaration tables, general function calls,
source-declared constructors, `match`, or proper tail calls.

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `gamma_evaluator.beta` | Beta-authored source for the current executable Gamma evaluator slice. | Replace only with the complete evaluator and synchronized Gamma gates. |