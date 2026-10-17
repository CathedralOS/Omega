# Nested borrowed parameter receiving slot

This is a source-only assignment-access control with no program entry or
runtime oracle. The incoming mutable reference lends the inner descriptor
slot. Replacing that slot requires the outer loan's mutable access, while the
replacement descriptor retains its shared pointee access. It does not assert
support for constructing a reference to a reference-valued local at a caller.
