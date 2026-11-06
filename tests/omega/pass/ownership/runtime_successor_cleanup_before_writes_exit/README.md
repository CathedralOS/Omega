# Cleanup before a later call

The preparing helper owns the cleanup record. It captures the value 7, then
its normal completion runs the hook that adds 10. Only after that helper
returns does `entered` borrow the Counter, observe 17 and leave 29. The plain
record and nested Wrapper follow the same order; both selected branches run.
Native and Terminal execution must both exit 0.

Nested machines have ordinary invocation lifetimes. The earlier version kept
its cleanup in the caller while passing the suspended parent into `entered`;
its expectation came from the retired source-state semantics. The unchanged
17/29 oracle now follows an explicit completed helper scope. The
[direct-parent-loan control](../../../fail/borrows/terminal_call_keeps_caller_cleanup_loan/main.omg)
retains that earlier program and must reject while the caller's cleanup loan
is still active.

The existing `successor_argument_write_keeps_linear_debt` and
`terminal_successor_write_keeps_linear_debt` controls install a linear payload
before caller cleanup. Both must still reject abandonment. Completed argument
calls retain their mutation effects; a prior Empty observation cannot excuse
those writes or establish a future cleanup result.

The motivating Cathedral resident-placement outcome remains separate. This
fixture establishes cleanup ordering through explicit invocation completion;
it does not establish resident backing or profile publication.
