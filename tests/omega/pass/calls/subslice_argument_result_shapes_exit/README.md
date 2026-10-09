# Calls lend subslices whatever their result binds

Run `omega run --both tests/omega/pass/calls/subslice_argument_result_shapes_exit/main.omg`.

The authored exit is 0. A Unit call binds no result, a discarded or stored
scalar call binds a dense value, and a structural call binds a whole value;
none of these changes how a subslice argument is lent. The program lends an
inline subslice of a fixed array local and parameter, an inline subslice of a
view parameter, two local subslice bindings of a view
(`let middle: &mut [u8] = view[2..4]`) and a whole view. Unit calls run between
scalar calls over the same storage, a discarded call, a call result stored into
a field and a structural result. The final reads check all sixteen bytes, so a
write that is skipped or lands at the wrong offset traps instead of exiting 0.
