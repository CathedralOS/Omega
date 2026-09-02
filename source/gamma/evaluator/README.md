# Gamma evaluator

This owner contains trusted Beta source for the Gamma evaluator. The admitted
Beta compiler translates it to Alpha tape, making this the Beta-to-Gamma
compiler edge rather than optional reconstruction tooling.

The current slice implements exact-ended request loading, source-envelope
validation, bounded unary function declaration rows, exact entry selection,
unary calls with isolated lexical environments, `if`, `let`, checked integer primitives,
all five byte primitives, total equality over its value universe,
`Complete`/`Reject`, immutable raw/hex/single/concat/slice nodes, linear local
lookup, bounded arena/stack collision checks, and terminal status classes. It
does not yet implement source-declared constructors, `match`, arbitrary function
arity, or proper tail calls. Ordinary recursion is bounded by the fixed evaluator
stack and reports `Incomplete` on collision.

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `gamma_evaluator.beta` | Beta-authored source for the current executable Gamma evaluator slice. | Replace only with the complete evaluator and synchronized Gamma gates. |