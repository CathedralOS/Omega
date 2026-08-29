use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError, StraightLineIntegerParameterTranslationError,
    lower_to_target_operations, validate_abstract_to_target_translation,
};
use omega_target_operations::TerminalPsiProvenance;

fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("test integer type")
}

fn parameter_return_plan(
    parameter_types: &[ScalarType],
    returned_parameter: usize,
) -> AbstractOperationPlan {
    let machine = MachineId::new(3_001).unwrap();
    let entry = BlockId::new(3_002).unwrap();
    let result_value = ValueId::new(3_003).unwrap();
    let return_edge = EdgeId::new(3_004).unwrap();
    let parameters = parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| AbstractParameter {
            value: ValueId::new(3_100 + index as u64).unwrap(),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let scalar_type = parameter_types[returned_parameter];
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: parameters.clone(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result_value,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![AbstractOperation::Return {
                psi_edge: return_edge,
                result: result_value,
                value: parameters[returned_parameter].value,
                scalar_type,
                cleanup_actions: Vec::new(),
            }],
        }],
    }
}

fn uniform_plan(integer: IntegerType, parameter_count: usize) -> AbstractOperationPlan {
    parameter_return_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 1,
    )
}

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerParameterTranslationError {
    let integer = integer_type(IntegerSign::Unsigned, 8);
    let mut source = uniform_plan(integer, 1);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_integer_parameter::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineIntegerParameterTranslationError {
    let integer = integer_type(IntegerSign::Unsigned, 8);
    let source = uniform_plan(integer, 1);
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut candidate);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerParameter,
        error: AbstractToTargetTranslationFamilyError::StraightLineIntegerParameter(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &candidate).unwrap_err()
    else {
        panic!("integer-parameter corruption must fail at its independent validator")
    };
    error
}

#[test]
fn validates_register_and_stack_parameter_returns_on_every_native_target() {
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
    let integer = integer_type(IntegerSign::Unsigned, 8);
    for ((target_profile, register), (_, stack)) in register_cases.into_iter().zip(stack_cases) {
        for (parameter_count, expected_location) in [(1, register), (9, stack)] {
            let source = uniform_plan(integer, parameter_count);
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            let receipt =
                validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
            let AbstractToTargetFunctionTranslationDisposition::Validated(
                AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerParameter(row),
            ) = receipt.function_roster()[0].translation()
            else {
                panic!("exact integer parameter return must publish its family receipt")
            };
            assert_eq!(row.machine(), source.entry);
            assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
            assert_eq!(row.parameter_index(), parameter_count - 1);
            assert_eq!(row.location(), expected_location);
            assert_eq!(row.scalar_type(), integer);
            assert!(matches!(
                target.functions[0].operation,
                TargetOperation::ReturnIntegerParameter { location, .. }
                    if location == expected_location
            ));
        }
    }
}

#[test]
fn validates_supported_integer_shapes_and_mixed_parameter_rosters() {
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        for bits in [8, 16, 32, 64] {
            let integer = integer_type(sign, bits);
            let source = uniform_plan(integer, 1);
            let target_profile = NativeTarget::linux_x64();
            let target = lower_to_target_operations(&source, target_profile).unwrap();
            validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
        }
    }

    let integer = integer_type(IntegerSign::Signed, 32);
    let source = parameter_return_plan(
        &[
            ScalarType::Boolean,
            ScalarType::Integer(integer),
            ScalarType::Boolean,
        ],
        1,
    );
    let target_profile = NativeTarget::linux_arm64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    let AbstractToTargetFunctionTranslationDisposition::Validated(
        AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerParameter(row),
    ) = receipt.function_roster()[0].translation()
    else {
        panic!("mixed scalar roster must retain the returned integer parameter")
    };
    assert_eq!(row.parameter_index(), 1);
    assert_eq!(row.source_value(), ValueId::new(3_101).unwrap());
}

#[test]
fn source_parameter_contract_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineIntegerParameterTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| function.parameters.push(function.parameters[0].clone())),
        StraightLineIntegerParameterTranslationError::SourceParameterRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(3_500).unwrap(),
                scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 24)),
            });
        }),
        StraightLineIntegerParameterTranslationError::SourceParameterShape
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[0] else {
                unreachable!()
            };
            *value = ValueId::new(3_501).unwrap();
        }),
        StraightLineIntegerParameterTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[0]
            else {
                unreachable!()
            };
            cleanup_actions.push(psi_terminal::TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(3_502).unwrap(),
            ));
        }),
        StraightLineIntegerParameterTranslationError::SourceCleanup
    );
}

#[test]
fn candidate_location_and_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineIntegerParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].operation = TargetOperation::ReturnIntegerParameter {
                psi_edge: EdgeId::new(3_004).unwrap(),
                source_value: ValueId::new(3_100).unwrap(),
                scalar_type: integer_type(IntegerSign::Unsigned, 8),
                parameter_index: 0,
                location: ScalarParameterLocation::IncomingStack { byte_offset: 0 },
            };
        }),
        StraightLineIntegerParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].operation = TargetOperation::ReturnIntegerImmediate {
                psi_edge: EdgeId::new(3_004).unwrap(),
                source_value: ValueId::new(3_100).unwrap(),
                scalar_type: integer_type(IntegerSign::Unsigned, 8),
                value: IntegerValue::Unsigned(0),
            };
        }),
        StraightLineIntegerParameterTranslationError::TargetOperation
    );
}
