# Separated range results

Run `omega run --both main.omg` after ordinary package preparation. Expected
native and Terminal exit: 0. The validator returns a qualified payload only for
nonempty, nonoverflowing, disjoint intervals, including either ordering and
adjacency. Overlap, containment, empty input and both overflow positions reject.
A loop rereads the same fields after an overlapping write and rejects the
overflowing second iteration; its internal state declares its numeric premise.

This isolates Cathedral's numeric root validation. It establishes neither
firmware-backed references nor a successful ExitBootServices handoff.
