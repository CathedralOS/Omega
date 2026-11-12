# Applied nominal invocation-local type arguments

Run `omega run --both tests/omega/pass/traits/applied_nominal_callback_closed_local_types_exit/main.omg`.

Two applications of `Visitor<Value>::visit<Context, Callback>` independently
bind their invocation-local context types to `u64` and `bool`. Their concrete
callbacks inspect both the nominal value and context. Both results must be true.
The selected visitor declarations rename the local type binder to `Kind`;
identity, not spelling or the enclosing machine's telescope, governs binding.

The separate pending `applied_nominal_callback_local_types_exit` case selects
generic callback declarations and still requires contextual closure of those
callbacks. This fixture does not claim that capability.
