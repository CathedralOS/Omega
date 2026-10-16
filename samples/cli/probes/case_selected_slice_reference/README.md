# Case-selected slice reference

A record lends a slice of its fixed array, then returns a mutable child selected
by a closed-sum transition. The caller writes through all three returned children
and checks the original array. Native exit 0 is the oracle; a wrong child or lost
write traps.

A second selector replaces its local slice with a view of another array before
returning. The caller checks that the returned child belongs to the replacement
and leaves neighboring elements unchanged.

This isolates the Dungeon Crawler's `Level::room_mut` custody join. The returned
borrow names the slice's referent type, while the local's produced carrier has
a reference type. Custody follows the exact local occurrence and captured loan.
