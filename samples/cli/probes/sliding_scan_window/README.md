# Sliding scan window

`omega run samples/cli/collections/sliding_scan_window/main.omg` must exit 0.
The helper advances a stored exclusive view through its original backing,
including a zero advance and an exhausted suffix. The program checks the
remaining records and neighboring storage rather than only the view's length.

Each helper call leaves one current descriptor with its possible exit origins.
A no-write route preserves the original backing, while a successful advance
updates the descriptor's address and extent. Neither route releases a detached
child or permits mutation of backing still held by the window.

Use `omega run --both samples/cli/collections/sliding_scan_window/main.omg`
to compare native execution with the Terminal interpreter; both must exit 0.
