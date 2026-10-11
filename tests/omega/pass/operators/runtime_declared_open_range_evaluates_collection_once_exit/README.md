# Declared open range captures its collection once

Acceptance: `omega run --both main.omg`. Both engines must exit 0.

The selected range body is an ordinary machine over a copied nominal collection.
Its omitted end reads that collection's captured length. The effectful producer
runs once in the selected Match arm and never in the unselected arm; the result
11 also witnesses the selected body rather than builtin window construction.

The explicit-end counterpart is `runtime_declared_range_guarded_collection_exit`.
The snapshot counterpart is `runtime_declared_open_range_preserves_operand_snapshot_exit`.
