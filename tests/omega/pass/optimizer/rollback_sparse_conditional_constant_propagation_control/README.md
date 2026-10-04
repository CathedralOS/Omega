# SparseConditionalConstantPropagation rollback, control leg

The control compiles the same `main.omg` as
`rollback_sparse_conditional_constant_propagation_selected` but authors no optimization selection.
Its `rollback.txt` still declares `SparseConditionalConstantPropagation`, which subtracts nothing: the
receipt records `applied=[]`, `module:same` shows the overlay was inert,
and its semantic fingerprints are the module the selected fixture's
`rolled-back-semantic` must equal.
