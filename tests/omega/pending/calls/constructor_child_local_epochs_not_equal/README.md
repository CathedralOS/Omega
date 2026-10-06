An immutable local holds constructor fields read before and after mutation.
The observer's equality requirement must be rejected. Both published baseline
6c0fd10e915 and the constructor-child capture candidate incorrectly accept
`omega --check tests/omega/pending/calls/constructor_child_local_epochs_not_equal/main.omg`.

The shared structural-term reader substitutes immutable initializers without
retaining their evaluation epochs. FlowStatementFact does not yet retain the
completed RHS operand tree, and statement construction transfers read a final
context. Retain the RHS capture for local initialization and assignment, carry
it through ordinary value-origin transfer, and consume that evidence instead of
replaying initializer syntax. A same-epoch stored pair must still prove equal;
immutability alone cannot identify the values its initializer observed.
