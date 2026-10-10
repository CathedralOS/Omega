# builtin_shadow_names

First sample shadowing compiler builtins with authored
machines: bare `clamp(3)` and `max(3)` select the
module's own one-arg declarations (results 4 and 43), not
the builtin `clamp`/`max` which want different arities.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
