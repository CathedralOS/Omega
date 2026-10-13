Closed scalar trait calls compose inside returned canonical `Complex<T>` records.
The nested product/sum result is `(1.875, 0)`; sibling applications with identical
signatures retain distinct `Double` and `Shifted` selections, yielding
`(0.5, 0.0625)` and `(1.5, 1.0625)`. Each exact numerical assertion executes.
The f32 carrier repeats the scalar composition, and an inherited default
requirement calls the selected identity row inside another returned record,
producing `(1.25, 2.25)`. These are executed assertions, not exit parity alone.
