# linear_bare_establish

`let mut receipt: Receipt;` on a `[linear]` type owns only zeroed
storage — `receipt = Receipt { code: 7 }` establishes the value and
creates the obligation; `ReceiptAck::ack(receipt)` transfers custody and
discharges it. Entry spelled `machine Main::main() -> i32` (context-free,
scalar result).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
