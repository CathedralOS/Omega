# static_namespace_constant

`Counter::constant()`/`Counter::generic_constant<T [copy]>(argument)` —
static machines on a data carrier's namespace see module-level `const`
but no field bindings; `[copy]` bounds the static generic's type param.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
