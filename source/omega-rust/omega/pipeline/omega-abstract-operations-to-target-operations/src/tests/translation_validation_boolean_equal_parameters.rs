use super::parameter_translation_fixture::{
    boolean_equal_parameters_plan, integer_type, uniform_boolean_equal_plan,
};
use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineBooleanEqualParametersTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};
use omega_target_operations::TerminalPsiProvenance;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineBooleanEqualParametersTranslationError {
    let mut source = uniform_boolean_equal_plan(2);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::boolean::equal::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineBooleanEqualParametersTranslationError {
    let source = uniform_boolean_equal_plan(2);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineBooleanEqualParameters,
        error: AbstractToTargetTranslationFamilyError::StraightLineBooleanEqualParameters(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("Boolean equality corruption must fail at its independent validator")
    };
    error
}

#[test]
fn validates_boolean_equality_register_and_stack_operands_on_every_native_target() {
    let cases = [
        (
            NativeTarget::linux_x64(),
            [
                ScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 16 },
                ScalarParameterLocation::IncomingStack { byte_offset: 24 },
            ],
        ),
        (
            NativeTarget::windows_x64(),
            [
                ScalarParameterLocation::Register(MachineRegister::X86Rcx),
                ScalarParameterLocation::Register(MachineRegister::X86Rdx),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 64 },
                ScalarParameterLocation::IncomingStack { byte_offset: 72 },
            ],
        ),
        (
            NativeTarget::uefi_x64(),
            [
                ScalarParameterLocation::Register(MachineRegister::X86Rcx),
                ScalarParameterLocation::Register(MachineRegister::X86Rdx),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 64 },
                ScalarParameterLocation::IncomingStack { byte_offset: 72 },
            ],
        ),
        (
            NativeTarget::linux_arm64(),
            [
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 0 },
                ScalarParameterLocation::IncomingStack { byte_offset: 8 },
            ],
        ),
        (
            NativeTarget::macos_arm64(),
            [
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                ScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ],
            [
                ScalarParameterLocation::IncomingStack { byte_offset: 0 },
                ScalarParameterLocation::IncomingStack { byte_offset: 8 },
            ],
        ),
    ];
    for (target_profile, registers, stack) in cases {
        for (parameter_count, expected) in [(2, registers), (10, stack)] {
            let source = uniform_boolean_equal_plan(parameter_count);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanEqualParameters(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("exact Boolean equality must publish its family receipt")
            };
            assert_eq!(row.machine(), source.entry);
            assert_eq!(row.equal_operation(), OperationId::new(3_800).unwrap());
            assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
            assert_eq!(row.source_value(), ValueId::new(3_801).unwrap());
            assert_eq!(row.left_parameter_index(), parameter_count - 2);
            assert_eq!(row.right_parameter_index(), parameter_count - 1);
            assert_eq!(row.left_location(), expected[0]);
            assert_eq!(row.right_location(), expected[1]);
        }
    }
}

#[test]
fn boolean_equality_retains_operand_order_and_identical_operand_identity() {
    for (left, right) in [(1, 0), (0, 0)] {
        let source =
            boolean_equal_parameters_plan(&[ScalarType::Boolean, ScalarType::Boolean], left, right);
        let target_profile = NativeTarget::linux_x64();
        let target = lower_to_target_operations(&source, target_profile).unwrap();
        let receipt =
            validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
        let AbstractToTargetFunctionTranslationDisposition::Validated(
            AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanEqualParameters(row),
        ) = receipt.function_roster()[0].translation()
        else {
            panic!("Boolean equality must retain ordered operand custody")
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
fn boolean_equality_source_identity_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::BooleanEqual { result, .. } = &mut function.operations[0] else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineBooleanEqualParametersTranslationError::SourceEqualResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanEqual { left, .. } = &mut function.operations[0] else {
                unreachable!()
            };
            *left = ValueId::new(3_900).unwrap();
        }),
        StraightLineBooleanEqualParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineBooleanEqualParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineBooleanEqualParametersTranslationError::SourceReturnLink
    );
}

#[test]
fn boolean_equality_target_provenance_and_expression_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineBooleanEqualParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::Equal { left, .. } = expression else {
                unreachable!()
            };
            let TargetBooleanExpression::Parameter { location, .. } = left.as_mut() else {
                unreachable!()
            };
            *location = ScalarParameterLocation::IncomingStack { byte_offset: 0 };
        }),
        StraightLineBooleanEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *expression = TargetBooleanExpression::Parameter {
                source_value: ValueId::new(3_100).unwrap(),
                parameter_index: 0,
                location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            };
        }),
        StraightLineBooleanEqualParametersTranslationError::TargetOperation
    );
}
