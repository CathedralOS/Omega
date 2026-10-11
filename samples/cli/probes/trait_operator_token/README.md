# trait_operator_token

Structural scalar trait operators: a trait crowns a fixed token on a
named requirement (`operator < before`), a generic caller resolves
the token through its evidence binder (`left < right` in
`choose<Element, Order satisfies Ranked<Element>>`), the requirement
stays callable by name (`Ascending::before`), and a concrete
`machine <` crowns the canonical token meaning for its operand
signature.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
