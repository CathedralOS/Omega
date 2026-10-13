Run `omega run --both tests/omega/pass/drops/runtime_selected_transfer_fieldwise_construction_exit/main.omg`.
A structural local declared without an initializer (`let mut pair: Pair<Token>;`)
whose zero bits are not a value (`Token.value` is ranged to `[1..=100]`) is a
deferred binding; its field stores establish it one field at a time, and only
established fields owe cleanup. A selected (`&&`-guarded) consuming call of
one field after the construction must transfer that field out of the deferred
binding and keep the sibling's residual cleanup and the ordinary reverse
declaration order at normal death, in both engines, with exit 0 and the
authored hook order.

Each line is one call: the consumed token's hook prints inside the callee (`A`),
then at the test machine's exit the youngest local (`C`) and the pair's
remaining fields in reverse declaration order (`B`, then `A` when retained).

The fixture explicitly assumes finite normal completion of its reviewed byte-write
provider and binds real libc/MSVCRT imports, like
`runtime_selected_partial_owner_cleanup_exit`.
