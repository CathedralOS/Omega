# A parent view is used again after its exclusive child's last use

Run `omega run --both tests/omega/pass/borrows/local_subslice_parent_lent_after_last_use_exit/main.omg`.

The authored exit is 0. `middle`, `left` and `right` are exclusive subslices of
the local view `view`. `middle` is lent to a call; after its last use the
program reads `view[3]` as a call operand. `left` and `right` are disjoint, so
each is lent while the other is live; after both last uses the program lends
`view` shared and then exclusively. The final reads check all eight bytes, so
a call that skipped or misplaced a write traps instead of exiting 0. The same
parent uses before a live child's last use are rejected by the
`fail/borrows/local_subslice_live_parent_*` fixtures.
