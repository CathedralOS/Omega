use super::parameter_translation_fixture::{
    integer_bitwise_not_parameter_plan, integer_type, uniform_integer_bitwise_not_plan,
};
use super::*;
use crate::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamily, AbstractToTargetTranslationFamilyError,
    AbstractToTargetTranslationValidationError,
    StraightLineIntegerBitwiseNotParameterTranslationError, lower_to_target_operations,
    validate_abstract_to_target_translation,
};
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{ClaimId, ServiceId, StructuralDomainId};
use psi_terminal::EntryClaim;

fn leaf_error(
    mutate: impl FnOnce(&mut AbstractFunction),
) -> StraightLineIntegerBitwiseNotParameterTranslationError {
    let mut source = uniform_integer_bitwise_not_plan(integer_type(IntegerSign::Signed, 32), 1);
    let target_profile = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut source.functions[0]);
    crate::validation::straight_line_parameter::integer::unary::bitwise_not::validate(
        &source.functions[0],
        target_profile,
        &target.functions[0],
    )
    .unwrap_err()
}

fn candidate_error(
    mutate: impl FnOnce(&mut omega_target_operations::TargetOperationPlan),
) -> StraightLineIntegerBitwiseNotParameterTranslationError {
    let source = uniform_integer_bitwise_not_plan(integer_type(IntegerSign::Unsigned, 64), 1);
    let target_profile = NativeTarget::linux_x64();
    let mut target = lower_to_target_operations(&source, target_profile).unwrap();
    mutate(&mut target);
    let AbstractToTargetTranslationValidationError::FunctionFamily {
        family: AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotParameter,
        error: AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseNotParameter(error),
        ..
    } = validate_abstract_to_target_translation(&source, target_profile, &target).unwrap_err()
    else {
        panic!("integer bitwise-not corruption must fail at its independent validator")
    };
    error
}

#[test]
fn validates_integer_bitwise_not_types_registers_and_stack_on_every_native_target() {
    let target_cases = [
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
    for (target_profile, register, stack) in target_cases {
        for scalar_type in integers {
            for (parameter_count, expected_location) in [(1, register), (9, stack)] {
                let source = uniform_integer_bitwise_not_plan(scalar_type, parameter_count);
                let target = lower_to_target_operations(&source, target_profile).unwrap();
                let receipt =
                    validate_abstract_to_target_translation(&source, target_profile, &target)
                        .unwrap();
                let AbstractToTargetFunctionTranslationDisposition::Validated(
                    AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotParameter(
                        row,
                    ),
                ) = receipt.function_roster()[0].translation()
                else {
                    panic!("exact integer bitwise-not must publish its family receipt")
                };
                assert_eq!(row.machine(), source.entry);
                assert_eq!(
                    row.bitwise_not_operation(),
                    OperationId::new(4_200).unwrap()
                );
                assert_eq!(row.return_edge(), EdgeId::new(3_004).unwrap());
                assert_eq!(row.source_value(), ValueId::new(4_201).unwrap());
                assert_eq!(row.scalar_type(), scalar_type);
                assert_eq!(
                    row.operand_value(),
                    ValueId::new(3_100 + parameter_count as u64 - 1).unwrap()
                );
                assert_eq!(row.parameter_index(), parameter_count - 1);
                assert_eq!(row.location(), expected_location);
            }
        }
    }
}

#[test]
fn integer_bitwise_not_retains_exact_mixed_roster_operand_custody() {
    let integer = integer_type(IntegerSign::Signed, 32);
    let source = integer_bitwise_not_parameter_plan(
        &[
            ScalarType::Boolean,
            ScalarType::Integer(integer_type(IntegerSign::Unsigned, 8)),
            ScalarType::Integer(integer),
            ScalarType::Boolean,
        ],
        2,
    );
    let target_profile = NativeTarget::linux_arm64();
    let target = lower_to_target_operations(&source, target_profile).unwrap();
    let receipt =
        validate_abstract_to_target_translation(&source, target_profile, &target).unwrap();
    let AbstractToTargetFunctionTranslationDisposition::Validated(
        AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotParameter(row),
    ) = receipt.function_roster()[0].translation()
    else {
        panic!("mixed roster must retain the exact integer bitwise-not operand")
    };
    assert_eq!(row.scalar_type(), integer);
    assert_eq!(row.operand_value(), ValueId::new(3_102).unwrap());
    assert_eq!(row.parameter_index(), 2);
    assert_eq!(
        row.location(),
        ScalarParameterLocation::Register(MachineRegister::Aarch64X(2))
    );
}

#[test]
fn integer_bitwise_not_source_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::IntegerBitwiseNot { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceBitwiseNotResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseNot { operand, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *operand = ValueId::new(42_999).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractFunctionResult::Scalar(result) = &mut function.result else {
                unreachable!()
            };
            result.scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters.push(AbstractParameter {
                value: ValueId::new(42_998).unwrap(),
                scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned, 24)),
            });
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceParameterShape
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
                PlaceId::new(42_997).unwrap(),
            ));
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceCleanup
    );
}

#[test]
fn integer_bitwise_not_shared_source_envelope_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(42_910).unwrap(),
                    position: 0,
                    is_self: false,
                    structural_type: StructuralTypeId::new(42_911).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: vec![StructuralDomainId::new(42_912).unwrap()],
                });
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceStructuralParameters
    );
    assert_eq!(
        leaf_error(|function| {
            function.entry_claims.push(EntryClaim {
                claim: ClaimId::new(42_913).unwrap(),
                input: PlaceId::new(42_914).unwrap(),
                path: Vec::new(),
            });
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceEntryClaims
    );
    assert_eq!(
        leaf_error(|function| {
            function
                .published_service_ceiling
                .push(ServiceId::new(42_915).unwrap());
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourcePublishedServices
    );
    assert_eq!(
        leaf_error(|function| function.block_entries.clear()),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceBlockRoster
    );
    assert_eq!(
        leaf_error(|function| function.parameters.push(function.parameters[0].clone())),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceParameterRoster
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperationRoster
    );
}

#[test]
fn integer_bitwise_not_target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.clear();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.edges.clear();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetProvenance
    );
}

#[test]
fn integer_bitwise_not_target_expression_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { psi_edge, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(43_000).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { scalar_type, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Signed, 64);
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { source_value, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *source_value = ValueId::new(43_001).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot { psi_operation, .. } = expression else {
                unreachable!()
            };
            *psi_operation = OperationId::new(43_002).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot { operand, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter { source_value, .. } = operand.as_mut() else {
                unreachable!()
            };
            *source_value = ValueId::new(43_003).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot { operand, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter {
                parameter_index, ..
            } = operand.as_mut()
            else {
                unreachable!()
            };
            *parameter_index = 1;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot { operand, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter { location, .. } = operand.as_mut() else {
                unreachable!()
            };
            *location = ScalarParameterLocation::IncomingStack { byte_offset: 0 };
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot {
                psi_operation,
                operand,
            } = expression
            else {
                unreachable!()
            };
            *expression = TargetIntegerExpression::IntegerWiden {
                psi_operation: *psi_operation,
                source_type: integer_type(IntegerSign::Unsigned, 32),
                operand: operand.clone(),
            };
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].operation = TargetOperation::ReturnIntegerParameter {
                psi_edge: EdgeId::new(3_004).unwrap(),
                source_value: ValueId::new(4_201).unwrap(),
                scalar_type: integer_type(IntegerSign::Unsigned, 64),
                parameter_index: 0,
                location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            };
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
}
