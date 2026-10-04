# ProofCheckElision rollback, control leg

The control compiles the same `main.omg` as
`rollback_proof_check_elision_selected` but authors no optimization selection.
Its `rollback.txt` still declares `ProofCheckElision`, which subtracts nothing: the
receipt records `applied=[]`, `module:same` shows the overlay was inert,
and its semantic fingerprints are the module the selected fixture's
`rolled-back-semantic` must equal.
