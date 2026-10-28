The enum arm lends an iteration-local array to a machine in its recursive
component. The checked transfer keeps that array alive until the receiving step
has read it. The consumer verifies all sixteen iterations; the other enum arm
returns a scalar normally.

Callable discovery must not resurrect the completing call as a standalone entry:
that false entry would need custody of an invocation-local loan it never received.
Only the exact successor occurrence is excluded. The `forwarded(index)` call in
its operand still needs its ordinary callable body and must execute.

This extends the Cathedral v4 client regression in `native_component_case_cycle`.
The component passes both Terminal interpretation and native execution.
