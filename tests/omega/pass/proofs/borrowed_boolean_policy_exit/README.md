# Use a helper’s check of a referenced field

A caller subtracts 37 from a referenced field after a helper confirms that the field equals 37. The program calls the helper with fields containing 37 and 91. The matching branch must return 0 and the other branch 91; `main` traps if either result is wrong.

The helper promises `accepts(result, value)`, a total Boolean computation meaning that a true result implies the referenced field equals 37. The compiler already expanded that computation, but skipped judging its resulting Boolean expression. The repair uses the existing observed-predicate judgment and preserves the completed Boolean result when deriving the conditional integer fact. It keeps the reference’s observation and storage version; it does not turn a reference into an unrestricted mathematical value.

The [wrong-validator twin](../../../fail/proofs/borrowed_boolean_policy_wrong_validator/main.omg) returns true unconditionally and must fail the helper’s guarantee. The [changed-storage twin](../../../fail/proofs/borrowed_boolean_policy_changed_storage/main.omg) writes 91 after the check and must fail the caller’s requirement. The old true result cannot authorize a changed field.

This is a prerequisite for Cathedral’s checked interrupt-table publication. It demonstrates an ordinary helper’s borrowed-field guarantee, not the separate selected-provider contract, interrupt installation, or timer delivery.
