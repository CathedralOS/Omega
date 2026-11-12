The Integer addition overflows under Trapping and takes its permitted Trap
route while evaluating the call argument. The outer `finite_value` call never
executes. The program must trap in native and Terminal execution.

The sealed conversion law bounds any completed Integer value and can establish
finiteness on normal return without inventing a value for an overflowing
operation. This follows the normal-return law in
[proof contracts](../../../../../wiki/spec/proofs/contracts.md) and the runtime
Trapping rule in [numeric values](../../../../../wiki/spec/language/numeric_values.md).
The negative `symbolic_integer_conversion_requires_preserves_trapping_failure`
control uses `crashes Trap false` and must reject the uncovered source operation.
