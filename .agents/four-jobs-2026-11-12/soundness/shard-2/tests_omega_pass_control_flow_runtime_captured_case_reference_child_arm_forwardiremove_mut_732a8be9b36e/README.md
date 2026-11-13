# Captured case reference retains its child's lineage

After `omega update --project . --dangerously-accept-all`,
`omega run --both main.omg` must report native and interpreter exit 70.
A runtime-indexed shared descriptor establishes a child before dispatch. The
selected arm forwards a reborrow of that child and the successor observes the
original referent again. No arm operand names the parent descriptor directly.
Its tag observation is not permission to release it while the child remains
live: checked cleanup must preserve deepest-first lineage closure. Neither
descriptor outlives or mutates the array it borrows.
