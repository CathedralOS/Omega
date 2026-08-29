use super::parameter_translation_fixture::{
    integer_less_or_equal_parameters_plan, integer_type, uniform_integer_less_or_equal_plan,
};
use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineIntegerLessOrEqualParametersTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};
use omega_target_operations::TerminalPsiProvenance;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerLessOrEqualParametersTranslationError {
    let mut source = uniform_integer_less_or_equal_plan(integer_type(IntegerSign::Signed, 32), 2);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer_less_or_equal::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineIntegerLessOrEqualParametersTranslationError {
    let source = uniform_integer_less_or_equal_plan(integer_type(IntegerSign::Unsigned, 64), 2);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualParameters,
        error:
            AbstractToTargetTranslationFamilyError::StraightLineIntegerLessOrEqualParameters(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("integer less-or-equal corruption must fail at its independent validator")
    };
    error
}

#[test]
fn validates_integer_less_or_equal_types_registers_and_stack_on_every_native_target() {
    let targets = [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ];
    let integers = [
        integer_type(IntegerSign::Signed, 8),
        integer_type(IntegerSign::Unsigned, 8),
        integer_type(IntegerSign::Signed, 16),
        integer_type(IntegerSign::Unsigned, 16),
        integer_type(IntegerSign::Signed, 32),
        integer_type(IntegerSign::Unsigned, 32),
        integer_type(IntegerSign::Signed, 64),
        integer_type(IntegerSign::Unsigned, 64),
    ];
    for target_profile in targets {
        for scalar_type in integers {
            for parameter_count in [2, 10] {
                let source = uniform_integer_less_or_equal_plan(scalar_type, parameter_count);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLessOrEqualParameters(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("exact integer less-or-equal must publish its family receipt")
                };
                assert_eq!(
                    row.less_or_equal_operation(),
                    OperationId::new(4_100).unwrap()
                );
                assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
                assert_eq!(row.source_value(), ValueId::new(4_101).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(row.left_parameter_index(), parameter_count - 2);
                assert_eq!(row.right_parameter_index(), parameter_count - 1);
                assert_ne!(row.left_location(), row.right_location());
            }
        }
    }
}

#[test]
fn integer_less_or_equal_retains_order_identity_and_mixed_roster_custody() {
    let integer = integer_type(IntegerSign::Unsigned, 32);
    for (left, right) in [(2, 1), (1, 1)] {
        let source = integer_less_or_equal_parameters_plan(
            &[
                ScalarType::Boolean,
                ScalarType::Integer(integer),
                ScalarType::Integer(integer),
            ],
            left,
            right,
        );
        let target_profile = NativeTarget::linux_x64();
        let target = lower_to_target_operations(&source, target_profile).unwrap();
        let receipt =
            validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLessOrEqualParameters(
                row,
            ),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("integer less-or-equal must retain ordered operand custody")
        };
        assert_eq!(row.left_parameter_index(), left);
        assert_eq!(row.right_parameter_index(), right);
        assert_eq!(row.left_value(), source.functions[0].parameters[left].value);
        assert_eq!(
            row.right_value(),
            source.functions[0].parameters[right].value
        );
    }
}

#[test]
fn integer_less_or_equal_source_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::IntegerLessOrEqual { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceLessOrEqualResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(integer_type(IntegerSign::Signed, 32));
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let (psi_operation, result, left, right) = match &function.operations[0] {
                AbstractOperation::IntegerLessOrEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                } => (*psi_operation, *result, *left, *right),
                _ => unreachable!(),
            };
            function.operations[0] = AbstractOperation::IntegerLessThan {
                psi_operation,
                result,
                left,
                right,
            };
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceOperationRoster
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
                PlaceId::new(4_200).unwrap(),
            ));
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceCleanup
    );
}

#[test]
fn integer_less_or_equal_target_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual { scalar_type, .. } = expression else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Signed, 64);
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression {
                source_value,
                expression: _,
                ..
            } = &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *source_value = ValueId::new(49_999).unwrap();
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter {
                location: left_location,
                ..
            } = left.as_ref()
            else {
                unreachable!()
            };
            let left_location = *left_location;
            let TargetIntegerExpression::Parameter {
                location: right_location,
                ..
            } = right.as_mut()
            else {
                unreachable!()
            };
            *right_location = left_location;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } = expression else {
                unreachable!()
            };
            std::mem::swap(left, right);
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual { left, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter {
                parameter_index, ..
            } = left.as_mut()
            else {
                unreachable!()
            };
            *parameter_index = 1;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual {
                psi_operation,
                scalar_type,
                left,
                right,
            } = expression
            else {
                unreachable!()
            };
            *expression = TargetBooleanExpression::IntegerLessThan {
                psi_operation: *psi_operation,
                scalar_type: *scalar_type,
                left: left.clone(),
                right: right.clone(),
            };
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
}
