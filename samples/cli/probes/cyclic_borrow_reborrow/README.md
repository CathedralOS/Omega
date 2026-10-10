# cyclic_borrow_reborrow

A `&mut` machine argument rebinds through an unranked cycle's block-parameter
channel (`go`/`step` passing `sink`), then re-borrows whole into a unit
machine call `Sink::bump(sink)` — a call-scoped loan created and released
inside the operation.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
