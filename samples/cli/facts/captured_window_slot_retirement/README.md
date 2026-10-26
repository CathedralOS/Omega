# Captured window descriptor retirement

Replacing the exact captured window descriptor releases its displaced backing while retaining replacement and disjoint sibling loans.

Parent: **BORROWED-STORAGE-RESTORATION** in [TASKS.md](../../../../TASKS.md).
The governing laws are [ownership windows](../../../../wiki/spec/language/ownership.md#borrowed-storage-invariant-windows),
[assignment evaluation](../../../../wiki/spec/language/expressions.md#evaluation-schedule),
and [Terminal restoration](../../../../wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).

Two windows initially borrow distinct one-cell arrays. Replacing element zero's
slice descriptor installs a two-cell replacement. The caller then mutates the
displaced original backing to 9, observes its unchanged tag 17, reads replacement
values/tags 42/7 and 19/29, and verifies the untouched neighbor 31/41. The store
captures its address before the RHS. Retirement consumes that operation's exact
coordinate; it does not dispose either borrowed backing or invent rollback.

The selector travels through local, field, parameter, call result, match join,
constant, another state, another machine, and target-selected body positions.
Each twin changes only the descriptor store's selector to a completed call that
returns one. That store cannot release the original backing retained by window
zero. The required rejection names the original backing's active window loan.
Constants are scalar selectors; they do not own runtime reference carriers.

The existing `run/borrows/proved_window_index_replacement` exposed the gap:
its local, field and parameter routes already source-checked, but an inline
completed selector left the original loan active after replacing its descriptor.
The frozen pre-repair producer rejects the isolated inline call with two active
loan diagnostics. After source repair, executable production separately refused
the structural destination and its descriptor cleanup. The repair retains both
coordinates of the same operation: the runtime selector for execution and the
completed capture for authenticating the exact historical descriptor occurrence.

Additional controls retain wrong/unknown selectors, the neighbor's loan, the
replacement's new loan, and a descriptor used for a write-through operation.
An effectful selector returns zero once, changes a cursor to one, and an RHS
producer changes it to three; both engines must still replace element zero.

All nine development positives execute native 0 and Terminal 0 on Linux x86-64;
all nine wrong-slot twins reject the intended active loan. Final qualification
and independent receipt review accompany the PR. Other target bindings do not
claim native execution on those hosts. The whole TASKS.md outcome remains open.
