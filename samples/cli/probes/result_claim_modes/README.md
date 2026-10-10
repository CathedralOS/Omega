# result_claim_modes

First sample contrasting result-claim modes: `make` returns a
self-established receipt (fresh claim in the caller's
namespace) while `pass` returns its input unchanged (the claim
continues the transferred identity). A `Receipt in
Receipt::Issued` domain qualification continues across both.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
