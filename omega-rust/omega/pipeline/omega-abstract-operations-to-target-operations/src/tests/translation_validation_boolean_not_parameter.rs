use super::parameter_translation_fixture::{
    boolean_not_parameter_plan, integer_type, uniform_boolean_not_plan,
};
use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineBooleanNotParameterTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};
use omega_target_operations::TerminalPsiProvenance;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineBooleanNotParameterTranslationError {
    let mut source = uniform_boolean_not_plan(1);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::boolean::not::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineBooleanNotParameterTranslationError {
    let source = uniform_boolean_not_plan(1);
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineBooleanNotParameter,
        error: AbstractToTargetTranslationFamilyError::StraightLineBooleanNotParameter(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("Boolean-not-parameter corruption must fail at its independent validator")
    };
    error
}

#[test]
fn validates_boolean_not_parameter_register_and_stack_on_every_native_target() {
    let cases = [
        (
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
        ),
        (
            NativeTarget::windows_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::uefi_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
        (
            NativeTarget::macos_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ];
    for (target_profile, register, stack) in cases {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let source = uniform_boolean_not_plan(parameter_count);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanNotParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("exact Boolean-not parameter must publish its family receipt")
            };
            assert_eq!(row.machine(), source.entry);
            assert_eq!(row.not_operation(), OperationId::new(3_700).unwrap());
            assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
            assert_eq!(row.source_value(), ValueId::new(3_701).unwrap());
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                target.functions[0].operation,
                TargetOperation::ReturnBooleanNotParameter { location, .. }
                    if location == expected_location
            ));
        }
    }
}

#[test]
fn validates_boolean_not_from_a_mixed_scalar_roster() {
    let integer = integer_type(IntegerSign::Unsigned, 32);
    let source = boolean_not_parameter_plan(
        &[
            ScalarType::Integer(integer),
            ScalarType::Boolean,
            ScalarType::Integer(integer),
        ],
        1,
    );
    let target_profile = NativeTarget::linux_arm64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    let AbstractToTargetFunctionTranslationDisposition::Validated(
        AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanNotParameter(row),
    ) = receipt.function_roster()[0].translation()
    else {
        panic!("mixed roster must retain Boolean-not operand custody")
    };
    assert_eq!(row.operand_value(), ValueId::new(3_101).unwrap());
    assert_eq!(row.parameter_index(), 1);
}

#[test]
fn boolean_not_source_identity_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::BooleanNot { result, .. } = &mut function.operations[0] else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineBooleanNotParameterTranslationError::SourceNotResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::BooleanNot { operand, .. } = &mut function.operations[0] else {
                unreachable!()
            };
            *operand = ValueId::new(3_702).unwrap();
        }),
        StraightLineBooleanNotParameterTranslationError::SourceOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineBooleanNotParameterTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(3_703).unwrap(),
                scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 24)),
            });
        }),
        StraightLineBooleanNotParameterTranslationError::SourceParameterShape
    );
}

#[test]
fn boolean_not_candidate_provenance_and_operation_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineBooleanNotParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].operation = TargetOperation::ReturnBooleanNotParameter {
                psi_edge: EdgeId::new(3_004).unwrap(),
                source_value: ValueId::new(3_701).unwrap(),
                parameter_index: 0,
                location: ScalarParameterLocation::IncomingStack { byte_offset: 0 },
            };
        }),
        StraightLineBooleanNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].operation = TargetOperation::ReturnBooleanParameter {
                psi_edge: EdgeId::new(3_004).unwrap(),
                source_value: ValueId::new(3_701).unwrap(),
                parameter_index: 0,
                location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            };
        }),
        StraightLineBooleanNotParameterTranslationError::TargetOperation
    );
}
