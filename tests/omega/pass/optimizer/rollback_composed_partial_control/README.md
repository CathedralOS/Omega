# Partial rollback, control leg

The control compiles the same `main.omg` as
`rollback_composed_partial_selected` but authors only
`DeadPureScalarElimination`. Its `rollback.txt` still declares
`CopyPropagation`, which subtracts nothing: the receipt records
`applied=[]`, `module:same` shows the overlay was inert, and its
semantic fingerprints are the module the selected fixture's
`rolled-back-semantic` must equal.
