# Beta evaluator Alpha source

This tool owner contains readable Alpha Tape Assembly for constructing and
auditing in-progress Beta evaluator slices. It is not a compiler edge and does
not establish authority for a future canonical evaluator tape.

The current slice implements exact-ended request loading, source-envelope
validation, one unary entry function, `if`, `let`, checked integer primitives,
all five byte primitives, total equality over its value universe,
`Complete`/`Reject`, immutable raw/hex/single/concat/slice nodes, linear local
lookup, bounded arena/stack collision checks, and terminal status classes. It
does not yet implement declaration tables, general function calls,
source-declared constructors, `match`, or proper tail calls.

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `evaluator.alphaasm` | Hand-authored source for the current executable Beta evaluator slice. | Delete only when replaced by the complete audited evaluator source or when direct byte audit no longer benefits from readable reconstruction. |