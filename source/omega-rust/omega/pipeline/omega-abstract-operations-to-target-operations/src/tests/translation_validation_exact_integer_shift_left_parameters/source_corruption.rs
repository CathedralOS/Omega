use super::{base_source, exact_integer_shift_left_parameters_plan, leaf_error};
use crate::StraightLineExactIntegerShiftLeftParametersTranslationError;
use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use omega_target::NativeTarget;
use psi_core::{IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, ValueId};

#[test]
fn exact_shift_left_source_identity_and_independent_type_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(
            |function| super::super::scalar_result_mut(function).scalar_type = ScalarType::Boolean
        ),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::ExactIntegerShiftLeft { result, .. } =
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
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceShiftResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerShiftLeft { value, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *value = ValueId::new(55_500).unwrap();
        }),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceValueOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerShiftLeft { count, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *count = ValueId::new(55_501).unwrap();
        }),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceCountOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type =
                ScalarType::Integer(super::integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceValueTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerShiftLeft { count_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *count_type = super::integer_type(IntegerSign::Signed, 16);
        }),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceCountTypeMismatch
    );
    for malformed in [
        super::integer_type(IntegerSign::Unsigned, 6),
        super::integer_type(IntegerSign::Signed, 24),
    ] {
        assert_eq!(
            malformed_type_error(malformed, false),
            StraightLineExactIntegerShiftLeftParametersTranslationError::SourceParameterShape
        );
        assert_eq!(
            malformed_type_error(malformed, true),
            StraightLineExactIntegerShiftLeftParametersTranslationError::SourceParameterShape
        );
    }
    assert_eq!(
        leaf_error(|function| set_address_carrier(function, false)),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceValueCarrier
    );
    assert_eq!(
        leaf_error(|function| set_address_carrier(function, true)),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceCountCarrier
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerShiftLeft { obligation, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(55_506).unwrap();
        }),
        StraightLineExactIntegerShiftLeftParametersTranslationError::TargetOperation
    );
}

#[test]
fn exact_shift_left_source_semantics_order_return_and_cleanup_corruption_fails_closed() {
    for substitute in 1..=21 {
        assert_eq!(
            leaf_error(|function| replace_shift(function, substitute)),
            StraightLineExactIntegerShiftLeftParametersTranslationError::SourceOperationRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceReturnLink
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
                PlaceId::new(55_502).unwrap(),
            ));
        }),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineExactIntegerShiftLeftParametersTranslationError::SourceOperationRoster
    );

    let integer = super::integer_type(IntegerSign::Signed, 32);
    let mut source = exact_integer_shift_left_parameters_plan(
        &[ScalarType::Integer(integer), ScalarType::Integer(integer)],
        0,
        1,
    );
    let target_profile = NativeTarget::linux_x64();
    let target = crate::lower_to_target_operations(&source, target_profile).unwrap();
    let AbstractOperation::ExactIntegerShiftLeft { value, count, .. } =
        &mut source.functions[0].operations[0]
    else {
        unreachable!()
    };
    std::mem::swap(value, count);
    assert_eq!(
        crate::validation::straight_line_parameter::integer::shift::exact_left::validate(
            &source.functions[0],
            target_profile,
            &target.functions[0],
        )
        .unwrap_err(),
        StraightLineExactIntegerShiftLeftParametersTranslationError::TargetOperation
    );
}

fn malformed_type_error(
    malformed: IntegerType,
    mutate_count: bool,
) -> StraightLineExactIntegerShiftLeftParametersTranslationError {
    let mut source = base_source();
    let parameter = if mutate_count { 1 } else { 0 };
    source.functions[0].parameters[parameter].scalar_type = ScalarType::Integer(malformed);
    let AbstractOperation::ExactIntegerShiftLeft {
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
    let target = omega_target_operations::TargetFunction {
        machine: source.functions[0].machine,
        attachment: source.functions[0].attachment,
        fixed_integer_scalar_abi: None,
        provenance: omega_target_operations::TerminalPsiProvenance::default(),
        operation: omega_target_operations::TargetOperation::ReturnIntegerImmediate {
            psi_edge: psi_core::EdgeId::new(55_503).unwrap(),
            source_value: psi_core::ValueId::new(55_504).unwrap(),
            scalar_type: malformed,
            value: psi_core::IntegerValue::Unsigned(0),
        },
    };
    crate::validation::straight_line_parameter::integer::shift::exact_left::validate(
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
    let AbstractOperation::ExactIntegerShiftLeft {
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
    let AbstractOperation::ExactIntegerShiftLeft {
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
