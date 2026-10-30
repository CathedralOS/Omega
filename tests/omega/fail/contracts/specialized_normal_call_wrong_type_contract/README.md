# A normal result cannot acquire another type application's contract

Only the ensured application changes from `choose<u32>` to `choose<u64>`;
the actual call and runtime oracle remain unchanged. Both frozen R23 and R24
refuse this exact ensures. R24's matching positive executes in both engines.

[Positive and retained evidence](../../../pass/contracts/runtime_specialized_normal_call_contract_exit/README.md).
