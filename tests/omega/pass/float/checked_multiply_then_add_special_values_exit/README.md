# Checked multiply-then-add realization

Requirement: target-realized canonical float arithmetic, established through a
selected checked machine under the public operation contract.

The selected checked machine must round multiplication before addition for both
binary32 and binary64. The witness covers an underflow tie, the normal/subnormal
boundary, overflow cancellation, negative zero, invalid infinity operations and
NaN propagation. NaN assertions classify the value; they do not claim a payload.
The unchanged [named cancellation witness](../named_provider_multiply_then_add_exit/main.omg)
continues to distinguish fused and unfused results.

`main` has expected exit 0 on every hosted target. `omega run --both` must observe
native and Terminal agreement. The existing `boundary-provider-controls` check
requires a demanded checked body, re-admits retained and decoded Terminal products
after releasing the source session, and rejects missing selection or substitution
of the public requirement for its body. This core-only program has no other
checked service provider that could satisfy that control.

The [wrong-argument contract control](../../../fail/float/checked_float_provider_contract_argument_order/main.omg)
keeps the body and its own guarantee consistent while exchanging multiplier and
addend; it must fail the canonical operation's unchanged contract. The checked
body's call, storage and machine-state plans use ordinary provider realization.
The selected realization report must retain its actual identity and the target's
ISA and floating-control obligations.

Governing contract: [numeric values](../../../../../wiki/spec/language/numeric_values.md#target-realization-and-control-state).
Qualification records identify the exact producer and actual target; merely
compiling the other target bindings is not hardware execution evidence.
