# Reads outside a live exclusive loan stay available

Run `omega run --both tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg`.

The authored exit is 0. Each read observes storage no live exclusive loan
suspends: an element outside the branch a live subslice captured, the parent
view and the owner after their children's last uses, a scalar after its `&mut`
loan's last use, a field disjoint from a live `&mut` field loan, and storage
held only by a shared loan. The final guard checks every value, including the
writes made through `zero(tail)`, `borrowed` and `left`, so a misplaced or
skipped write traps instead of exiting 0. The same reads while the exclusive
loan is still live are rejected by
`fail/borrows/plain_read_during_live_exclusive_loan`.
