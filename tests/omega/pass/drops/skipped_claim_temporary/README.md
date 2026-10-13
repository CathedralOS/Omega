# skipped_claim_temporary

No owner is established on a statically skipped evaluation path:
`false && inspect(Pair { first: Receipt {..}, second: Receipt {..} })`
never constructs the claim-bearing temporaries, so neither linear member
needs an owner or cleanup.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
