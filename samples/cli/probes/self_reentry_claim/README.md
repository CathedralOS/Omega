# self_reentry_claim

First sample with a linear claim surviving a state-cycle
backedge: `spin(receipt, left)` reenters its own entry holding
the receipt until the count runs out — each iteration's entry
claim continues the arriving edge's claim.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
