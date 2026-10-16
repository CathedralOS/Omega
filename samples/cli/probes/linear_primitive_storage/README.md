# Primitive storage with live claims

Expected exit: 0. The program owns a linear receipt while assembly writes
unrelated primitive local storage in the entry state and a named state.
Both writes preserve the receipt until its owner-authored explicit drop.
A failed scalar check follows the declared Trap route.

Run `omega run samples/cli/probes/linear_primitive_storage/main.omg`.
The build root binds each supported hosted entry.
