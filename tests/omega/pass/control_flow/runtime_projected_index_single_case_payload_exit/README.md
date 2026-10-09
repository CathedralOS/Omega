# Runtime-indexed one-case payload dispatch

After `omega update --project . --dangerously-accept-all`,
`omega run --both main.omg` must report native and interpreter exit 70.
The selector captures the second element, and its two payload scalars are
snapshotted before the successor regains mutable access to the original root.
Reading the first element produces 3 instead of 70.
