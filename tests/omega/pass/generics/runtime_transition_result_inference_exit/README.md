# Named-state receiving context

`omega run --both main.omg` must exit 0 in both engines. The receiving state's
`[u8; 3]` parameter determines the nested `empty()` call's const argument.
The call returns all three zero bytes; no caller guard supplies a generic
argument or repairs the receiving contract.

`fail/generics/transition_result_cannot_reselect_explicit_extent` fixes the
callee argument at 2 and must reject: destination inference cannot overwrite
an explicit argument. This is a general call/state parameter join, not a
fixed-array-specific inference path.
