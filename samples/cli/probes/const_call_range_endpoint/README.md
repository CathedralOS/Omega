# const_call_range_endpoint

A machine call supplies a const range endpoint:
`u64 [0..=selected::limit()]` evaluates `selected::limit()` at
declaration time — same-spelled machines in other modules are not it.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
