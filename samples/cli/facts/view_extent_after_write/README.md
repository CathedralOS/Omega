# View extent after a content write

A view of at least 1024 bytes retains its extent after an indexed byte write
through a helper. Every place reads `view[view.len - 4]`, writes the first byte
through the helper, then restores the saved suffix byte using the same extent.
Each program exits 0 only when the first-byte change and suffix value reach the
caller. Its `invalid` twin weakens only the lower-bound fact to three bytes;
exact subtraction must still reject.

This witnesses the descriptor/content distinction needed by
FILESYSTEM-PLATFORM-SERVICES: Darwin directory enumeration needs to preserve its
private four-byte flags suffix around a native write into caller storage. It
does not authorize resizing, descriptor-slot replacement, stale content facts,
or unchecked indexing.

Run a place with `omega run samples/cli/facts/view_extent_after_write/parameter/main.omg`.
Check its control with `omega --check samples/cli/facts/view_extent_after_write/parameter/invalid/main.omg`.
