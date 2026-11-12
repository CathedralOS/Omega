# Selected range length requirement

`omega run --both main.omg` exits 0. Each selected matrix row has its own declared extent, so its checked `0..1` range supplies a nonempty descriptor to the callee. The original selector is captured before a later argument resets the cursor.
