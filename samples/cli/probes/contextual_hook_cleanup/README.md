# contextual_hook_cleanup

Contextual cleanup: temporaries constructed as call arguments owe the
selected hook's prerequisite at the exact owner. `Guard` and `Token`
share the same hook shape but retain distinct cleanup actions, run after
the call that consumed them.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
