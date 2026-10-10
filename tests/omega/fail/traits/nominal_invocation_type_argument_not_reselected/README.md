# A callback cannot reselect an explicit invocation-local type

`Routine<u64, wrong_context>` fixes `Context` to `u64` before admitting its
callback. The callback requires `bool`; it cannot infer a replacement context.
The generic consumer is uncalled but its authored body must still be checked.
