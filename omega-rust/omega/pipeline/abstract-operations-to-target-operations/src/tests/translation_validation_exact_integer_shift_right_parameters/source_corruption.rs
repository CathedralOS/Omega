use super::{base_source, exact_integer_shift_right_parameters_plan, leaf_error};
use crate::StraightLineExactIntegerShiftRightParametersTranslationError;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, ValueId};
use target::NativeTarget;

#[test]
fn exact_shift_right_source_identity_and_independent_type_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(
            |function| super::super::scalar_result_mut(function).scalar_type = ScalarType::Boolean
        ),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::ExactIntegerShiftRight { result, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceShiftResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerShiftRight { value, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *value = ValueId::new(55_500).unwrap();
        }),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceValueOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerShiftRight { count, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *count = ValueId::new(55_501).unwrap();
        }),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceCountOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type =
                ScalarType::Integer(super::integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceValueTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerShiftRight { count_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *count_type = super::integer_type(IntegerSign::Signed, 16);
        }),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceCountTypeMismatch
    );
    for malformed in [
        super::integer_type(IntegerSign::Unsigned, 6),
        super::integer_type(IntegerSign::Signed, 24),
    ] {
        assert_eq!(
            malformed_type_error(malformed, false),
            StraightLineExactIntegerShiftRightParametersTranslationError::SourceParameterShape
        );
        assert_eq!(
            malformed_type_error(malformed, true),
            StraightLineExactIntegerShiftRightParametersTranslationError::SourceParameterShape
        );
    }
    assert_eq!(
        leaf_error(|function| set_address_carrier(function, false)),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceValueCarrier
    );
    assert_eq!(
        leaf_error(|function| set_address_carrier(function, true)),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceCountCarrier
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerShiftRight { obligation, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(55_506).unwrap();
        }),
        StraightLineExactIntegerShiftRightParametersTranslationError::TargetOperation
    );
}

#[test]
fn exact_shift_right_source_semantics_order_return_and_cleanup_corruption_fails_closed() {
    for substitute in 1..=21 {
        assert_eq!(
            leaf_error(|function| replace_shift(function, substitute)),
            StraightLineExactIntegerShiftRightParametersTranslationError::SourceOperationRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(55_502).unwrap(),
            ));
        }),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineExactIntegerShiftRightParametersTranslationError::SourceOperationRoster
    );

    let integer = super::integer_type(IntegerSign::Signed, 32);
    let mut source = exact_integer_shift_right_parameters_plan(
        &[ScalarType::Integer(integer), ScalarType::Integer(integer)],
        0,
        1,
    );
    let target_profile = NativeTarget::linux_x64();
    let target = crate::lower_to_target_operations(&source, target_profile).unwrap();
    let AbstractOperation::ExactIntegerShiftRight { value, count, .. } =
        &mut source.functions[0].operations[0]
    else {
        unreachable!()
    };
    std::mem::swap(value, count);
    assert_eq!(
        crate::validation::straight_line_parameter::integer::shift::exact_right::validate(
            &source.functions[0],
            target_profile,
            &target.functions[0],
        )
        .unwrap_err(),
        StraightLineExactIntegerShiftRightParametersTranslationError::TargetOperation
    );
}

fn malformed_type_error(
    malformed: IntegerType,
    mutate_count: bool,
) -> StraightLineExactIntegerShiftRightParametersTranslationError {
    let mut source = base_source();
    let parameter = if mutate_count { 1 } else { 0 };
    source.functions[0].parameters[parameter].scalar_type = ScalarType::Integer(malformed);
    let AbstractOperation::ExactIntegerShiftRight {
        value_type,
        count_type,
        ..
    } = &mut source.functions[0].operations[0]
    else {
        unreachable!()
    };
    if mutate_count {
        *count_type = malformed;
    } else {
        *value_type = malformed;
        let AbstractOperation::Return { scalar_type, .. } = &mut source.functions[0].operations[1]
        else {
            unreachable!()
        };
        *scalar_type = ScalarType::Integer(malformed);
        super::super::scalar_result_mut(&mut source.functions[0]).scalar_type =
            ScalarType::Integer(malformed);
    }
    let target_profile = NativeTarget::linux_x64();
    let target = crate::lower_to_target_operations(&source, target_profile).unwrap_err();
    assert!(matches!(
        target,
        crate::LoweringError::ParameterWidthNotNativelySupported { .. }
            | crate::LoweringError::ExactShiftOperandTypeMismatch(_)
    ));
    let target = target_operations::TargetFunction {
        machine: source.functions[0].machine,
        attachment: source.functions[0].attachment,
        fixed_integer_scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: target_operations::TerminalPsiProvenance::default(),
        operation: target_operations::TargetOperation::ReturnIntegerImmediate {
            psi_edge: semantic_vocabulary::EdgeId::new(55_503).unwrap(),
            source_value: semantic_vocabulary::ValueId::new(55_504).unwrap(),
            scalar_type: malformed,
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
        },
    };
    crate::validation::straight_line_parameter::integer::shift::exact_right::validate(
        &source.functions[0],
        target_profile,
        &target,
    )
    .unwrap_err()
}

fn set_address_carrier(function: &mut AbstractFunction, mutate_count: bool) {
    let address = IntegerType::address(64).unwrap();
    let parameter = if mutate_count { 1 } else { 0 };
    function.parameters[parameter].scalar_type = ScalarType::Integer(address);
    let AbstractOperation::ExactIntegerShiftRight {
        value_type,
        count_type,
        ..
    } = &mut function.operations[0]
    else {
        unreachable!()
    };
    if mutate_count {
        *count_type = address;
    } else {
        *value_type = address;
        let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
            unreachable!()
        };
        *scalar_type = ScalarType::Integer(address);
        super::super::scalar_result_mut(function).scalar_type = ScalarType::Integer(address);
    }
}

fn replace_shift(function: &mut AbstractFunction, substitute: u8) {
    let AbstractOperation::ExactIntegerShiftRight {
        psi_operation,
        obligation,
        result,
        value_type,
        count_type,
        value,
        count,
    } = function.operations[0]
    else {
        unreachable!()
    };
    function.operations[0] = match substitute {
        1 => AbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        },
        2 => AbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        },
        3 => AbstractOperation::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        },
        4 => AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        5 => AbstractOperation::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        6 => AbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        7 => AbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        8 => AbstractOperation::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        9 => AbstractOperation::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        10 => AbstractOperation::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        11 => AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        12 => AbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        13 => AbstractOperation::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        14 => AbstractOperation::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        15 => AbstractOperation::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        16 => AbstractOperation::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        17 => AbstractOperation::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        18 => AbstractOperation::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        19 => AbstractOperation::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        20 => AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        21 => AbstractOperation::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
        },
        _ => unreachable!(),
    };
}
