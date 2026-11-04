# Use the helper selected by a trait

A selected checker confirms that a referenced field equals 37, allowing its caller to subtract 37. The matching input returns 0; the input 91 takes the other branch and returns 91. Both results are checked at runtime.

`Check<Cell, accepts>` names the Boolean helper that describes the checker’s guarantee. Previously the compiler left the abstract `Valid` name in the copied contract, so it rejected an implementation that correctly promised `accepts(result, value)`. The repair substitutes the selected declaration while retaining the original binder and its contract checks.

The [attached-helper variant](../selected_boolean_policy_attached_exit/main.omg) selects `Cell::accepts` and exercises its declaration receiver. The [wrong-validator twin](../../../fail/proofs/selected_boolean_policy_wrong_validator/main.omg) changes the checker’s body to unconditional true and must fail its guarantee.

The [configured callback control](../../generics/selected_boolean_policy_applied_contract_exit/main.omg) already passed before this repair. Its copied precondition and body use helpers configured with distinct constants 37 and 91. Its [invalid twin](../../../fail/generics/selected_boolean_policy_applied_contract_wrong_argument/main.omg) changes one argument to 38 and must reject the resulting false precondition. These controls preserve the separate complete callback-application path.

This completes another prerequisite for Cathedral’s interrupt-table validator. Installed roots, acknowledgement creation and actual timer delivery remain separate unfinished acceptance.
