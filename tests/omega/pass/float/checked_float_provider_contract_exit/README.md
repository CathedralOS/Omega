# Checked float provider contracts

`TARGET-REALIZED-FLOAT-ARITHMETIC` needs checked Omega bodies that satisfy the
canonical float contracts. These forwarding bodies publish the same MTA
meaning through the public `F32::meaning` and `F64::meaning` projections. Their
exported `Identity` guarantees connect them to the private projections used by
the canonical requirement; provider matching must consume those guarantees.

The historical producer at `16399564053987af0ceb3d5bc70e1cc778cd0843` proved the
body guarantees but rejected their correspondence with the requirement. The
identical source files and observation are retained in
`build/checked-float-forwarding` in the continuation worktree. This is distinct
from the still-open target-table checked-machine entry and software MTA proof.

The positive expects exit 0 in both engines. Its
[reject control](../../../fail/float/checked_float_provider_contract_argument_order/main.omg)
proves a different, correctly forwarded MTA guarantee after exchanging the
multiplier and addend. That guarantee must not satisfy the original requirement.

Authority: [numeric contracts](../../../../../wiki/spec/language/numeric_values.md#target-realization-and-control-state)
and [explicit identity](../../../../../wiki/spec/proofs/contracts.md#data-identity-in-facts).
