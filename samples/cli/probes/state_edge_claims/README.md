# state_edge_claims

First sample with claims crossing nested-machine edges: a receipt
established in the entry state moves into an inline `Pair`
construction, crosses a conditional edge into one of two
states, and each state's entry claim continues the claim its
arriving edge carried.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
