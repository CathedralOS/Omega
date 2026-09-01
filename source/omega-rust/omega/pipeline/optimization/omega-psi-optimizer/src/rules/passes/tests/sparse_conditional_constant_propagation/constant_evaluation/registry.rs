//! Exact SCCP catalog positions for constant-evaluation rule families.

use super::*;

#[test]
fn sccp_registry_pins_every_binary_integer_constant_rule_position() {
    let registry =
        registry_for_optimization(Optimization::SparseConditionalConstantPropagation).unwrap();
    let contracts = registry.contracts().collect::<Vec<_>>();
    let expected = [
        (
            ExactIntegerAddConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            ExactIntegerSubtractConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            ExactIntegerMultiplyConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            WrappingIntegerAddConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            WrappingIntegerSubtractConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            WrappingIntegerMultiplyConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            SaturatingIntegerAddConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            SaturatingIntegerSubtractConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            SaturatingIntegerMultiplyConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            ExactIntegerDivideConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            ExactIntegerRemainderConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            WrappingIntegerDivideConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            WrappingIntegerRemainderConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            SaturatingIntegerDivideConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            SaturatingIntegerRemainderConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            ExactIntegerShiftLeftConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            ExactIntegerShiftRightConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            WrappingIntegerShiftLeftConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            WrappingIntegerShiftRightConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            IntegerBitwiseAndConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            IntegerBitwiseOrConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            IntegerBitwiseXorConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
    ];
    let positions = (0..=18).chain(22..=24);
    for (position, (expected, safety)) in positions.zip(expected) {
        assert_eq!(
            contracts[position].identity(),
            expected.identity(),
            "binary integer constant rule at SCCP position {position}",
        );
        assert_eq!(
            contracts[position].safety_class(),
            safety,
            "binary integer constant-rule safety at SCCP position {position}",
        );
    }
}

#[test]
fn sccp_registry_pins_every_unary_integer_constant_rule_position() {
    let registry =
        registry_for_optimization(Optimization::SparseConditionalConstantPropagation).unwrap();
    let contracts = registry.contracts().collect::<Vec<_>>();
    let expected = [
        (
            ExactIntegerCastConstantsRule::contract(),
            OptimizationSafetyClass::ProofCertified,
        ),
        (
            IntegerWidenConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        (
            IntegerBitwiseNotConstantsRule::contract(),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
    ];
    for (position, (expected, safety)) in (19..=21).zip(expected) {
        assert_eq!(
            contracts[position].identity(),
            expected.identity(),
            "unary integer constant rule at SCCP position {position}",
        );
        assert_eq!(
            contracts[position].safety_class(),
            safety,
            "unary integer constant-rule safety at SCCP position {position}",
        );
    }
}

#[test]
fn sccp_registry_places_boolean_result_constant_rules_before_range_rules() {
    let registry =
        registry_for_optimization(Optimization::SparseConditionalConstantPropagation).unwrap();
    let contracts = registry.contracts().collect::<Vec<_>>();
    assert_eq!(contracts.len(), 39);
    assert_eq!(
        contracts[25].identity(),
        BooleanNotConstantsRule::contract().identity()
    );
    assert_eq!(
        contracts[26].identity(),
        BooleanEqualConstantsRule::contract().identity()
    );
    assert_eq!(
        contracts[27].identity(),
        IntegerEqualConstantsRule::contract().identity()
    );
    assert_eq!(
        contracts[28].identity(),
        IntegerLessThanConstantsRule::contract().identity()
    );
    assert_eq!(
        contracts[29].identity(),
        IntegerLessOrEqualConstantsRule::contract().identity()
    );
}
