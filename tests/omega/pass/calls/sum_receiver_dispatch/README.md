# sum_receiver_dispatch

A carrier-qualified static machine may borrow a SUM, not just a
record: `Transition::equals(transition: &Transition, ...)` dispatches
on the borrowed sum's cases, and its states receive the discriminated
other explicitly — nominal ownership manufactures no record-shaped
receiver.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
