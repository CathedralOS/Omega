Run `omega run --both tests/omega/pass/calls/runtime_selected_call_argument_from_fieldwise_local_exit/main.omg`.
A structural local declared without an initializer (`let mut pair: Pair<Token>;`)
and established one field store at a time is the same call-argument home as a
local built from a whole literal. A selected (`&&`-guarded) consuming call of
one of its fields, after the construction or in the middle of it, must be owned
by the guard's computation rather than scheduled as a statement call. Both
engines exit 0; a wrong observation crashes with `Trap`.
