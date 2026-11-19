# case_payload_atomic

An atomic cell as a case payload mutates through the shared sum borrow:
`choice.counter.fetch_add` under `requires choice in Choice::Cell` — case
selection lends the original atomic cell, not a copy of its bits;
`ensures result in Choice::Cell` carries the domain into callers.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
