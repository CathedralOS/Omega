# Named input retains its own bytes

The corpus supplies the `template` slot through `BuildSnapshotRequest::with_inputs`.
The build reads its immutable `input.txt`, checks the returned bytes, then seals
and completes one required output. A package-source file with the same relative
name contains different bytes, so selecting the wrong root cannot succeed.
Opening a package-only member through the slot must also fail without fallback.

The shipped CLI has no named-slot assignment flag. Running this fixture directly
must refuse the missing `template` slot, not read the package-source file instead.
