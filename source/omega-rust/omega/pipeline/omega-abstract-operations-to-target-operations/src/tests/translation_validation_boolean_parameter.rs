use super::parameter_translation_fixture::{
    integer_type, parameter_return_plan, uniform_boolean_plan,
};
use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineBooleanParameterTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};
use omega_target_operations::TerminalPsiProvenance;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineBooleanParameterTranslationError {
    let mut source = uniform_boolean_plan(1);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::boolean::direct::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineBooleanParameterTranslationError {
    let source = uniform_boolean_plan(1);
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineBooleanParameter,
        error: AbstractToTargetTranslationFamilyError::StraightLineBooleanParameter(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("Boolean-parameter corruption must fail at its independent validator")
    };
    error
}

#[test]
fn validates_register_and_stack_boolean_parameter_returns_on_every_native_target() {
    let register_cases = [
        (
            NativeTarget::linux_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        ),
        (
            NativeTarget::windows_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
        ),
        (
            NativeTarget::uefi_x64(),
            ScalarParameterLocation::Register(MachineRegister::X86Rcx),
        ),
        (
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
        ),
        (
            NativeTarget::macos_arm64(),
            ScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
        ),
    ];
    let stack_cases = [
        (
            NativeTarget::linux_x64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 16 },
        ),
        (
            NativeTarget::windows_x64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::uefi_x64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 64 },
        ),
        (
            NativeTarget::linux_arm64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
        (
            NativeTarget::macos_arm64(),
            ScalarParameterLocation::IncomingStack { byte_offset: 0 },
        ),
    ];
    for ((target_profile, register), (_, stack)) in register_cases.into_iter().zip(stack_cases) {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let source = uniform_boolean_plan(parameter_count);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("exact Boolean parameter return must publish its family receipt")
            };
            assert_eq!(row.machine(), source.entry);
            assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert!(matches!(
                target.functions[0].operation,
                TargetOperation::ReturnBooleanParameter { location, .. }
                    if location == expected_location
            ));
        }
    }
}

#[test]
fn validates_a_boolean_return_from_a_mixed_scalar_roster() {
    let integer = integer_type(IntegerSign::Signed, 32);
    let source = parameter_return_plan(
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
        AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanParameter(row),
    ) = receipt.function_roster()[0].translation()
    else {
        panic!("mixed scalar roster must retain the returned Boolean parameter")
    };
    assert_eq!(row.parameter_index(), 1);
    assert_eq!(row.source_value(), ValueId::new(3_101).unwrap());
}

#[test]
fn boolean_parameter_source_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.push(function.parameters[0])),
        StraightLineBooleanParameterTranslationError::SourceParameterRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(3_600).unwrap(),
                scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 24)),
            });
        }),
        StraightLineBooleanParameterTranslationError::SourceParameterShape
    );
    assert_eq!(
        leaf_error(|function| {
            let integer = integer_type(IntegerSign::Unsigned, 8);
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: ValueId::new(3_003).unwrap(),
                scalar_type: ScalarType::Integer(integer),
            });
        }),
        StraightLineBooleanParameterTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[0] else {
                unreachable!()
            };
            *value = ValueId::new(3_601).unwrap();
        }),
        StraightLineBooleanParameterTranslationError::SourceReturnLink
    );
}

#[test]
fn boolean_parameter_candidate_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineBooleanParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].operation = TargetOperation::ReturnBooleanParameter {
                psi_edge: EdgeId::new(3_004).unwrap(),
                source_value: ValueId::new(3_100).unwrap(),
                parameter_index: 0,
                location: ScalarParameterLocation::IncomingStack { byte_offset: 0 },
            };
        }),
        StraightLineBooleanParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: EdgeId::new(3_004).unwrap(),
                source_value: ValueId::new(3_100).unwrap(),
                value: true,
            };
        }),
        StraightLineBooleanParameterTranslationError::TargetOperation
    );
}
