# Captured store destination

The assignment captures the first array through a stored reference. Its RHS
replaces that reference with a view of the second array, then returns the value
to write. The pending store must still update the first array, and the checker
must retain enough evidence to admit `require_written(first[0])`. The second
array remains unchanged. Expected exit: 0.

The same oracle covers a scalar record field, a retained dynamic array index,
and both paths through a stateful RHS. Each variant replaces the supplying
descriptor before returning the stored value. The destination's coordinates
and backing stay fixed; its old contents are not snapshotted.

Owned view carriers exercise the same replacement with runtime extents. A
length guard bounds the target before the RHS replaces the descriptor; an empty
view must take the non-writing branch. These carriers are owned: readable
borrowed-record calls retain their distinct incoming-roster restoration contract.

Byte views use the same capture rule. A helper returns a descriptor into a
nonzero-offset window of a nested array; the pending store changes only its
original byte, preserving both sentinels and the replacement array. Moving or
retiring the descriptor must leave that backing live. An empty byte window
takes the non-writing branch.

Run `omega run --both samples/cli/memory/captured_store_destination/main.omg`.
