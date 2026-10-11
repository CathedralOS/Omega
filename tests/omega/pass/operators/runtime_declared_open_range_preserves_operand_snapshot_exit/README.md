# Open range reads its captured operand snapshot

Acceptance: `omega run --both main.omg`. Both engines must exit 0.

A generic selected arm copies the collection before its start expression changes
the original length from 5 to 9. The omitted end must read 5 from that snapshot:
the selected body returns 55, not 59. The skipped arm runs no start expression.

The following named call deliberately authors two separate collection producers.
Both must run, returning 23 with the counter at 3. Cloning preserves shared
normalized handles within one occurrence, not structural equality between calls.
