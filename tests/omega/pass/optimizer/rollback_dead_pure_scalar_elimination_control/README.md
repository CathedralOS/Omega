# DeadPureScalarElimination rollback, control leg

The control compiles the same `main.omg` as
`rollback_dead_pure_scalar_elimination_selected` but authors no optimization selection.
Its `rollback.txt` still declares `DeadPureScalarElimination`, which subtracts nothing: the
receipt records `applied=[]`, `module:same` shows the overlay was inert,
and its semantic fingerprints are the module the selected fixture's
`rolled-back-semantic` must equal.
