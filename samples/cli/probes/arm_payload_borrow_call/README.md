# arm_payload_borrow_call

First sample borrowing an arm-bound case payload into
another machine's `&` parameter: `Sum::Found { p } ->
(Payload::read_at(&p, position))` — the destructured `p`
lends `&p` inside the arm result without moving.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
