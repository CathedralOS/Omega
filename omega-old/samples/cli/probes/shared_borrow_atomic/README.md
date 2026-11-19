# shared_borrow_atomic

Atomic operations mutate `cell.value` through a SHARED `&Cell` borrow — no
ordinary-write grant on the enclosing record: `store` and `fetch_or` through
`&Cell` receivers.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
