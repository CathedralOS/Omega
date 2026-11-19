# auto_borrow_structural_arg

First sample auto-borrowing a value argument at the call
site: `Payload::clone(payload)` passes the bound value
where `source: &Payload` is declared — the call loans
the binding, leaving it readable for a later
`Payload::read(payload)`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
