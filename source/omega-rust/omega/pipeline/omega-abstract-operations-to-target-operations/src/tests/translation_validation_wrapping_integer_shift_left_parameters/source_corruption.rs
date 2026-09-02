use super::{base_source, leaf_error, wrapping_integer_shift_left_parameters_plan};
use crate::StraightLineWrappingIntegerShiftLeftParametersTranslationError;
use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use omega_target::NativeTarget;
use psi_core::{IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, ValueId};

#[test]
fn wrapping_shift_left_source_identity_and_independent_type_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(
            |function| super::super::scalar_result_mut(function).scalar_type = ScalarType::Boolean
        ),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::WrappingIntegerShiftLeft { result, .. } =
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
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceShiftResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerShiftLeft { value, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *value = ValueId::new(53_500).unwrap();
        }),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceValueOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerShiftLeft { count, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *count = ValueId::new(53_501).unwrap();
        }),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceCountOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type =
                ScalarType::Integer(super::integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceValueTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerShiftLeft { count_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *count_type = super::integer_type(IntegerSign::Signed, 16);
        }),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceCountTypeMismatch
    );
    for malformed in [
        super::integer_type(IntegerSign::Unsigned, 6),
        super::integer_type(IntegerSign::Signed, 24),
    ] {
        assert_eq!(
            malformed_type_error(malformed, false),
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceParameterShape
        );
        assert_eq!(
            malformed_type_error(malformed, true),
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceParameterShape
        );
    }
}

#[test]
fn wrapping_shift_left_source_semantics_order_return_and_cleanup_corruption_fails_closed() {
    for substitute in 1..=21 {
        assert_eq!(
            leaf_error(|function| replace_shift(function, substitute)),
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceOperationRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            cleanup_actions.push(psi_terminal::TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(53_502).unwrap(),
            ));
        }),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceOperationRoster
    );

    let integer = super::integer_type(IntegerSign::Signed, 32);
    let mut source = wrapping_integer_shift_left_parameters_plan(
        &[ScalarType::Integer(integer), ScalarType::Integer(integer)],
        0,
        1,
    );
    let target_profile = NativeTarget::linux_x64();
    let target = crate::lower_to_target_operations(&source, target_profile).unwrap();
    let AbstractOperation::WrappingIntegerShiftLeft { value, count, .. } =
        &mut source.functions[0].operations[0]
    else {
        unreachable!()
    };
    std::mem::swap(value, count);
    assert_eq!(
        crate::validation::straight_line_parameter::integer::shift::wrapping_left::validate(
            &source.functions[0],
            target_profile,
            &target.functions[0],
        )
        .unwrap_err(),
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::TargetOperation
    );
}

fn malformed_type_error(
    malformed: IntegerType,
    mutate_count: bool,
) -> StraightLineWrappingIntegerShiftLeftParametersTranslationError {
    let mut source = base_source();
    let parameter = if mutate_count { 1 } else { 0 };
    source.functions[0].parameters[parameter].scalar_type = ScalarType::Integer(malformed);
    let AbstractOperation::WrappingIntegerShiftLeft {
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
            | crate::LoweringError::WrappingShiftOperandTypeMismatch(_)
    ));
    let target = omega_target_operations::TargetFunction {
        machine: source.functions[0].machine,
        attachment: source.functions[0].attachment,
        fixed_integer_scalar_abi: None,
mixed_structural_scalar_abi: None,
        provenance: omega_target_operations::TerminalPsiProvenance::default(),
        operation: omega_target_operations::TargetOperation::ReturnIntegerImmediate {
            psi_edge: psi_core::EdgeId::new(53_503).unwrap(),
            source_value: psi_core::ValueId::new(53_504).unwrap(),
            scalar_type: malformed,
            value: psi_core::IntegerValue::Unsigned(0),
        },
    };
    crate::validation::straight_line_parameter::integer::shift::wrapping_left::validate(
        &source.functions[0],
        target_profile,
        &target,
    )
    .unwrap_err()
}

fn replace_shift(function: &mut AbstractFunction, substitute: u8) {
    let AbstractOperation::WrappingIntegerShiftLeft {
        psi_operation,
        result,
        value_type,
        count_type,
        value,
        count,
    } = function.operations[0]
    else {
        unreachable!()
    };
    let obligation = ObligationId::new(53_505).unwrap();
    function.operations[0] = match substitute {
        1 => AbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        },
        2 => AbstractOperation::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        },
        3 => AbstractOperation::ExactIntegerShiftRight {
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
