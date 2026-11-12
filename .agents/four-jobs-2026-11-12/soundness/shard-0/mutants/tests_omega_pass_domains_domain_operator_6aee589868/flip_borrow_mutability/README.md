# domain_operator_add

Domain-owned binary operator `machine + Quantity::Additive::add` with `requires right in Quantity::Additive`; caller comma-requires `a in ..., b in ...` discharges both operand premise and operator contract.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
