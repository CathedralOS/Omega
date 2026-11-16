# Explicit field omission

The LOWERED-FORM-CHECKER requirement is to accept a final `..` in record and case literals while checking every requested zero value. The marker fills omitted runtime fields with zero; it does not grant domain membership, issuer permission, references, callable values or proof evidence. Literals without the marker must supply every runtime field.

[The positive](main.omg) constructs nested records, a selected second case and an omitted sum whose zero value uses its first declared case. Nonzero neighboring fields expose misplaced writes. Both match helpers are called with matching and fallback tags, including matching payloads under the wrong tag. A wrong value or tag traps. [The reject twin](../../../fail/literals/explicit_field_omission/main.omg) omits only the required nonzero key. Both programs bind all four hosted targets.

The compiler preserves the marker through parsing, typed copies, reflection and expression identity. Ordinary record/case construction supplies each omitted value and checks the complete aggregate requirements. Qualified fields use the same predicate and issuer checks as an explicit domain addition. Only the selected payload is constructed. Source `zero_value<T>()` remains outside executable expression support.

The governing rules are [explicit field omission](../../../../../wiki/spec/language/data_and_literals.md#explicit-field-omission), [default domains and zero initialization](../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization) and [explicit erased bindings](../../../../../wiki/spec/proofs/contracts.md#explicit-erased-bindings). Erased fields keep their existing construction rules.

Validation is pending the original native/Psi run and intended key rejection. Runtime observations will be limited to macOS arm64; the other hosted targets are declared but have not been executed.
