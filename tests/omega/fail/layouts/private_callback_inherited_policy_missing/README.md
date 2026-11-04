# Refuse a handler registration with no calling convention

A handler registration must say how native code passes arguments to the handler.
This control selects the parent interface, which makes no such choice, and must
reject with the diagnostic in `expected.txt`.

Apart from explanatory comments, its program differs from the positive fixture
at `tests/omega/pass/layouts/private_callback_inherited_policy_exit` only in the
registrar binder: it selects `Callback::call` instead of `TimerCallback::call`.
The named handler and the unused sibling are unchanged. Neither may supply the
missing choice on the registrar's behalf.
