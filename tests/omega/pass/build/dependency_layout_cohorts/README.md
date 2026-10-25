# Dependency layout inputs

`main.omg` keeps dependency and consumer geometry distinct through package
review, source checking, Terminal Psi and native execution. Both engines exit
zero when the dependency reads 34, its scoped observation reads 51, and the
consumer reads 51 from its separate twelve-byte representation.

The no-entry dependency exercises three different context obligations:

- Its exported `DependencyView` obtains physical query inputs from explicit
  layout capabilities. Its public policy's query has the same empty write
  frame before and after folding, so independent package review still agrees.
- The private `ScopedView` is used only by the Mac body. Its policy requires
  the eight-byte base layout there; an unused Windows body separately checks
  a four-byte recast under its own guard. Other hosted observations retain
  their authored bodies and the same result.
- `joint_layout_observation` recasts between identical representations whose
  context rosters have six and four members. Both operands must exist in the
  same context. Equal output bytes do not erase the latter policy's query
  dependency or promote it to target-invariant geometry.

`valid_disjoint.omg` runs with nonoverlapping exclusive views.
`invalid_bounds.omg` must reject the twelve-byte view over four bytes, and
`invalid_overlap.omg` must reject the overlapping exclusive loans. These
controls retain the original consumer operations and geometry.
