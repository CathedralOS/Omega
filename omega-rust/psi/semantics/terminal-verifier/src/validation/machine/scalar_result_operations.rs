//! Operations registered by their scalar result: the value it defines and
//! the operand types its kind requires.

use super::super::crash::substitute_crash_routes;
use super::super::structural_operations::validate_service_reach;
use super::super::{
    BTreeMap, IdRegistry, MachineId, ModuleError, OperationKind, ScalarTerm, ScalarType,
    TerminalMachine, TerminalModule, insert_unique, insert_value, propositions,
    validate_boolean_structural_field,
};
use semantic_vocabulary::ValueId;
use terminal_psi::Operation;

/// Registers a scalar-result operation's value and checks its operands
/// against the result type, per operation kind: primitive and
/// byte-sequence reads, constants, comparisons, Boolean and integer logic,
/// widening and casts, and the wrapping, saturating and exact integer
/// arithmetic with the obligations the exact forms carry.
pub(super) fn register_scalar_result_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &Operation,
    registry: &mut IdRegistry,
    value_types: &mut BTreeMap<ValueId, ScalarType>,
) -> Result<(), ModuleError> {
    let Some(result) = operation.result.scalar() else {
        return Err(ModuleError::ScalarOperationHasUnitResult(operation.id));
    };
    insert_value(
        value_types,
        &mut registry.values,
        result.id,
        result.scalar_type,
    )?;
    match operation.kind.clone() {
        OperationKind::PrimitiveScalarRead { source, path } => {
            if super::super::primitive_storage::read_type(
                module,
                machine,
                operation.id,
                source,
                &path,
            )? != result.scalar_type
            {
                return Err(ModuleError::InvalidPrimitiveScalarRead {
                    operation: operation.id,
                    place: source,
                });
            }
        }
        OperationKind::StructuralByteSequenceFieldLength { .. } => {
            super::super::structural_byte_sequence_fields::validate(module, machine, operation)?;
        }
        OperationKind::ByteSequenceLength { source } => {
            super::super::byte_sequence_length::validate(module, machine, operation, source)?;
        }
        OperationKind::ByteSequenceRead {
            source,
            length,
            obligation,
            ..
        } => {
            super::super::byte_sequence_read::validate(module, machine, operation, source, length)?;
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::CallUnit { .. }
        | OperationKind::WriteOnlyPrimitiveStore { .. }
        | OperationKind::WriteOnlyIndexedPrimitiveStore { .. }
        | OperationKind::EstablishReference { .. }
        | OperationKind::ReleaseReference { .. }
        | OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { .. }
        | OperationKind::ByteSequenceWrite { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterUnit { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. }
        | OperationKind::EstablishScalarCase { .. }
        | OperationKind::EstablishScalarArray { .. }
        | OperationKind::ByteSequenceSubslice { .. }
        | OperationKind::EstablishRecord { .. }
        | OperationKind::EstablishPrimitiveLocal { .. }
        | OperationKind::StoreDynamicDescriptor { .. }
        | OperationKind::MoveStructuralField { .. }
        | OperationKind::StoreStructuralField { .. }
        | OperationKind::PortWrite { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. } => {
            unreachable!("structural/effect operations were validated above")
        }
        OperationKind::BoundaryCall { .. } => {
            unreachable!("boundary calls were validated above")
        }
        OperationKind::Call {
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        } => {
            let callee = machines
                .get(&callee)
                .copied()
                .ok_or(ModuleError::UnknownCallTarget {
                    operation: operation.id,
                    callee,
                })?;
            validate_service_reach(
                operation.id,
                &machine.published_service_ceiling,
                &callee.published_service_ceiling,
            )?;
            if crash_continuations
                .windows(2)
                .any(|pair| pair[0].cause >= pair[1].cause)
            {
                return Err(ModuleError::NonCanonicalCallCrashContinuations(
                    operation.id,
                ));
            }
            let substitutions = callee
                .parameters
                .iter()
                .zip(&arguments)
                .map(|(parameter, argument)| {
                    (
                        parameter.id,
                        ScalarTerm::value(*argument, parameter.scalar_type),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let expected_crash_continuations =
                substitute_crash_routes(&callee.contract.crash_routes, &substitutions);
            if !super::super::crash::crash_routes_match(
                &crash_continuations,
                &expected_crash_continuations,
            ) {
                return Err(ModuleError::CallCrashContinuationsMismatch {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            super::super::crash::validate_call_crash_coverage(
                machine,
                &crash_continuations,
                operation.id,
            )?;
            if callee.structural_places.iter().any(|place| {
                !super::super::scalar_array::plain_return_source(module, callee, place.id)
                    && super::super::record::completed_source(module, callee, place.id).is_none()
            }) || !callee.content_entry_claims.is_empty()
                || !callee.content_identity_reshuffles.is_empty()
                || !callee.content_partition_compositions.is_empty()
                || callee
                    .contract
                    .requires
                    .iter()
                    .chain(
                        callee
                            .contract
                            .ensures
                            .iter()
                            .map(|clause| &clause.proposition),
                    )
                    .any(propositions::proposition_contains_content)
            {
                return Err(ModuleError::CallTargetHasStructuralContract {
                    operation: operation.id,
                    callee: callee.id,
                });
            }
            let Some(callee_result) = callee.result.scalar() else {
                return Err(ModuleError::CallTargetReturnsUnit {
                    operation: operation.id,
                    callee: callee.id,
                });
            };
            if operation.result.expect_scalar().scalar_type != callee_result.scalar_type {
                return Err(ModuleError::CallResultTypeMismatch {
                    operation: operation.id,
                    expected: callee_result.scalar_type,
                    actual: operation.result.expect_scalar().scalar_type,
                });
            }
            if requirement_obligations.len() != callee.contract.requires.len() {
                return Err(ModuleError::CallRequirementArityMismatch {
                    operation: operation.id,
                    expected: callee.contract.requires.len(),
                    actual: requirement_obligations.len(),
                });
            }
            for obligation in requirement_obligations {
                insert_unique(
                    &mut registry.obligations,
                    obligation,
                    ModuleError::DuplicateObligation,
                )?;
            }
        }
        OperationKind::IntegerConstant { value } => {
            let ScalarType::Integer(integer_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(ModuleError::IntegerConstantRequiresIntegerResult(
                    operation.id,
                ));
            };
            if !integer_type.admits(value) {
                return Err(ModuleError::IntegerConstantOutsideResultType(operation.id));
            }
        }
        OperationKind::BooleanConstant { .. } => {
            if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                return Err(ModuleError::BooleanConstantRequiresBooleanResult(
                    operation.id,
                ));
            }
        }
        OperationKind::IeeeFloatConstant { value } => {
            let expected = ScalarType::IeeeFloat(value.format());
            let actual = operation.result.expect_scalar().scalar_type;
            if actual != expected {
                return Err(ModuleError::IeeeFloatConstantResultTypeMismatch {
                    operation: operation.id,
                    expected,
                    actual,
                });
            }
        }
        OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::IeeeFloat(_)
            ) {
                return Err(ModuleError::IeeeFloatFusedMultiplyAddRequiresFloatResult(
                    operation.id,
                ));
            }
        }
        OperationKind::IeeeFloatCompare { .. } => {
            if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                return Err(ModuleError::IeeeFloatComparisonRequiresBooleanResult(
                    operation.id,
                ));
            }
        }
        OperationKind::StructuralCaseMembership {
            source,
            ref path,
            case,
        } => {
            super::super::structural_case_membership::validate(
                module, machine, operation, source, path, case,
            )?;
        }
        OperationKind::BooleanStructuralField {
            source,
            ref path,
            field,
        } => {
            if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                return Err(ModuleError::BooleanStructuralFieldRequiresBooleanResult(
                    operation.id,
                ));
            }
            validate_boolean_structural_field(module, machine, operation.id, source, path, field)?;
        }
        OperationKind::IntegerStructuralField {
            source,
            ref path,
            field,
        } => {
            super::super::structural_scalar_fields::validate_integer_structural_field(
                module,
                machine,
                operation.id,
                source,
                path,
                field,
                operation.result.expect_scalar().scalar_type,
            )?;
        }
        OperationKind::BooleanNot { .. } => {
            if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                return Err(ModuleError::BooleanNotRequiresBooleanResult(operation.id));
            }
        }
        OperationKind::BooleanEqual { .. } => {
            if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                return Err(ModuleError::BooleanEqualRequiresBooleanResult(operation.id));
            }
        }
        OperationKind::IntegerEqual { .. } => {
            if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                return Err(ModuleError::IntegerEqualRequiresBooleanResult(operation.id));
            }
        }
        OperationKind::IntegerLessThan { .. } | OperationKind::IntegerLessOrEqual { .. } => {
            if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
                return Err(ModuleError::IntegerOrderingRequiresBooleanResult(
                    operation.id,
                ));
            }
        }
        OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                    operation.id,
                ));
            }
        }
        OperationKind::IntegerBitwiseNot { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::IntegerBitwiseRequiresIntegerResult(
                    operation.id,
                ));
            }
        }
        OperationKind::IntegerWiden { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::IntegerWidenRequiresIntegerResult(operation.id));
            }
        }
        OperationKind::IntegerExactCast { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::IntegerExactCastRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::WrappingIntegerShiftRequiresIntegerResult(
                    operation.id,
                ));
            }
        }
        OperationKind::ExactIntegerShiftRight { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::ExactIntegerShiftRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::ExactIntegerShiftLeft { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::ExactIntegerShiftRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::ExactIntegerAdd { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::ExactIntegerAddRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::ExactIntegerSubtract { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::ExactIntegerSubtractRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::ExactIntegerMultiply { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::ExactIntegerMultiplyRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::ExactIntegerDivide { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::ExactIntegerDivideRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::ExactIntegerRemainder { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::ExactIntegerRemainderRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::WrappingIntegerDivide { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::WrappingIntegerDivideRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::WrappingIntegerRemainder { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::WrappingIntegerRemainderRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::SaturatingIntegerDivide { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::SaturatingIntegerDivideRequiresIntegerResult(
                    operation.id,
                ));
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::SaturatingIntegerRemainder { obligation, .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(
                    ModuleError::SaturatingIntegerRemainderRequiresIntegerResult(operation.id),
                );
            }
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        OperationKind::WrappingIntegerAdd { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::WrappingIntegerAddRequiresIntegerResult(
                    operation.id,
                ));
            }
        }
        OperationKind::SaturatingIntegerAdd { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::SaturatingIntegerAddRequiresIntegerResult(
                    operation.id,
                ));
            }
        }
        OperationKind::WrappingIntegerSubtract { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::WrappingIntegerSubtractRequiresIntegerResult(
                    operation.id,
                ));
            }
        }
        OperationKind::SaturatingIntegerSubtract { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::SaturatingIntegerSubtractRequiresIntegerResult(
                    operation.id,
                ));
            }
        }
        OperationKind::WrappingIntegerMultiply { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::WrappingIntegerMultiplyRequiresIntegerResult(
                    operation.id,
                ));
            }
        }
        OperationKind::SaturatingIntegerMultiply { .. } => {
            if !matches!(
                operation.result.expect_scalar().scalar_type,
                ScalarType::Integer(_)
            ) {
                return Err(ModuleError::SaturatingIntegerMultiplyRequiresIntegerResult(
                    operation.id,
                ));
            }
        }
    }
    Ok(())
}
