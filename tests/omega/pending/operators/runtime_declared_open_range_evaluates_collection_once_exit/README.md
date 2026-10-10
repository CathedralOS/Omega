# Declared open range captures its collection once

Acceptance: `omega run --both main.omg`. Both engines must exit 0.

Current preparation refuses the omitted end of the selected range declaration.
Expanding it to a second collection expression is not a valid repair: guarded
collection producers must still execute exactly once. The explicit-end
counterpart at `pass/operators/runtime_declared_range_guarded_collection_exit`
exercises shared and mutable producer loans within the selected arm. An omitted
end must observe the captured collection operand, not create another producer
or hoist its effects out of that arm.

The selected range body is an ordinary machine over a copied nominal collection.
Its omitted end reads that collection's length. An effectful collection producer
runs once in the selected Match arm and never in the unselected arm; deriving the
end must not invoke it again. The result 11 also witnesses the selected body,
rather than builtin window construction.
