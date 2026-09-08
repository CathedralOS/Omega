//! Eligibility for cyclic scalar work, primitive locals, views, and receivers.

use super::super::{block_views, byte_sequence_subslice, primitive_storage};
use super::*;

/// Eligibility carries no proof or dominance authority. The caller runs the
/// ordinary operand, view, successor, and frontier checks after this fence.
pub(super) fn eligible(module: &TerminalModule, machine: &TerminalMachine) -> bool {
    if machine
        .ranked_scc
        .as_ref()
        .is_some_and(|ranking| ranking.as_unsigned_countdown().is_some())
        || machine.result.structural().is_some()
        || !machine.entry_claims.is_empty()
        || !machine.content_entry_claims.is_empty()
        || !machine.content_identity_reshuffles.is_empty()
        || !machine.content_partition_compositions.is_empty()
        || machine.contract.requires.iter().any(|requirement| {
            super::super::proposition_observes_places(
                requirement,
                &machine
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.access == StructuralAccess::MutableBorrow)
                    .map(|parameter| parameter.place)
                    .collect::<Vec<_>>(),
            )
        })
        || machine.structural_parameters.iter().any(|parameter| {
            parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || !(persistent_receiver(module, parameter)
                    || (parameter.access == StructuralAccess::SharedBorrow
                        && module.structural_types.iter().any(|declaration| {
                            declaration.id == parameter.structural_type
                                && matches!(
                                    declaration.shape,
                                    StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BorrowedView
                                    )
                                )
                        })))
        })
        || machine.structural_places.iter().any(|place| {
            !matches!(
                place.kind,
                StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::BlockParameter { .. }
                    | StructuralPlaceKind::OperationResult { .. }
                    | StructuralPlaceKind::ByteSequenceLiteral { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
            )
        })
    {
        return false;
    }
    machine.blocks.iter().all(|block| {
        matches!(
            block.terminator,
            Terminator::Jump { .. }
                | Terminator::Conditional { .. }
                | Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::Crash { .. }
        ) && block
            .operations
            .iter()
            .all(|operation| match &operation.kind {
                OperationKind::PortWrite { .. } => operation.result == OperationResult::Unit,
                OperationKind::EstablishPrimitiveLocal { .. } => {
                    primitive_storage::validate_establishment(module, machine, operation).is_ok()
                }
                OperationKind::PrimitiveScalarRead { source } => {
                    operation.result.scalar().is_some_and(|result| {
                        primitive_storage::read_type(module, machine, operation.id, *source)
                            == Ok(result.scalar_type)
                    })
                }
                OperationKind::WriteOnlyPrimitiveStore { destination, .. } => {
                    operation.result == OperationResult::Unit
                        && primitive_storage::local_result(machine, *destination).is_some()
                        && primitive_storage::store_type(
                            module,
                            machine,
                            operation.id,
                            *destination,
                        )
                        .is_ok()
                }
                OperationKind::CallStructuralScalar {
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } => {
                    operation.result.scalar().is_some()
                        && !structural_arguments.is_empty()
                        && structural_arguments.iter().all(|argument| {
                            argument.path.is_empty()
                                && argument.access != StructuralAccess::Owned
                                && primitive_storage::local_result(machine, argument.place)
                                    .is_some()
                        })
                        && claim_transfers.is_empty()
                        && requirement_obligations.is_empty()
                        && crash_continuations.is_empty()
                }
                OperationKind::BoundaryCall {
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    operation.result == OperationResult::Unit
                        && completion_receipts.is_empty()
                        && structural_arguments.iter().all(|argument| {
                            argument.path.is_empty()
                                && argument.access == StructuralAccess::SharedBorrow
                                && (machine
                                    .structural_parameters
                                    .iter()
                                    .any(|parameter| parameter.place == argument.place)
                                    || block_views::parameter(machine, argument.place).is_some()
                                    || machine.structural_places.iter().any(|place| {
                                        place.id == argument.place
                                            && matches!(
                                                place.kind,
                                                StructuralPlaceKind::ByteSequenceLiteral { .. }
                                            )
                                    })
                                    || byte_sequence_subslice::borrowed_result(
                                        machine,
                                        argument.place,
                                    )
                                    .is_some())
                        })
                }
                OperationKind::CallUnit {
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } => {
                    operation.result == OperationResult::Unit
                        && structural_arguments.iter().all(|argument| {
                            argument.path.is_empty()
                                && ((argument.access == StructuralAccess::MutableBorrow
                                    && machine.structural_parameters.iter().any(|parameter| {
                                        parameter.place == argument.place
                                            && persistent_receiver(module, parameter)
                                    }))
                                    || (argument.access != StructuralAccess::Owned
                                        && primitive_storage::local_result(
                                            machine,
                                            argument.place,
                                        )
                                        .is_some())
                                    || (argument.access == StructuralAccess::SharedBorrow
                                        && super::super::byte_sequence_length::validate_source(
                                            module,
                                            machine,
                                            operation,
                                            argument.place,
                                            || ModuleError::InvalidByteSequenceLengthSource {
                                                operation: operation.id,
                                                source: argument.place,
                                            },
                                        )
                                        .is_ok()))
                        })
                        && claim_transfers.is_empty()
                        && requirement_obligations.is_empty()
                        && crash_continuations.is_empty()
                }
                OperationKind::ByteSequenceSubslice { .. } => {
                    operation.result.structural().is_some_and(|result| {
                        byte_sequence_subslice::borrowed_result(machine, result.place)
                            == Some(result)
                    })
                }
                OperationKind::ByteSequenceLength { .. }
                | OperationKind::StructuralByteSequenceFieldLength { .. }
                | OperationKind::ByteSequenceRead { .. }
                | OperationKind::IntegerStructuralField { .. }
                | OperationKind::BooleanStructuralField { .. } => {
                    operation.result.scalar().is_some()
                }
                OperationKind::StructuralScalarFieldStore { .. }
                | OperationKind::StructuralByteSequenceFieldStore { .. }
                | OperationKind::StructuralByteSequenceFieldByteStore { .. }
                | OperationKind::EstablishByteSequenceLiteral { .. } => {
                    operation.result == OperationResult::Unit
                }
                kind => operation.result.scalar().is_some() && pure_scalar(kind),
            })
    })
}

fn persistent_receiver(
    module: &TerminalModule,
    parameter: &StructuralParameterDeclaration,
) -> bool {
    parameter.is_self
        && parameter.access == StructuralAccess::MutableBorrow
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && module.structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        })
}

/// Keep the admitted operation family explicit: scalar result shape alone
/// cannot admit calls, structural observations, or future effectful operations.
fn pure_scalar(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::IntegerConstant { .. }
            | OperationKind::BooleanConstant { .. }
            | OperationKind::IeeeFloatConstant { .. }
            | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
            | OperationKind::BooleanNot { .. }
            | OperationKind::BooleanEqual { .. }
            | OperationKind::IntegerEqual { .. }
            | OperationKind::IntegerLessThan { .. }
            | OperationKind::IntegerLessOrEqual { .. }
            | OperationKind::IntegerBitwiseNot { .. }
            | OperationKind::IntegerWiden { .. }
            | OperationKind::IntegerExactCast { .. }
            | OperationKind::IntegerBitwiseAnd { .. }
            | OperationKind::IntegerBitwiseOr { .. }
            | OperationKind::IntegerBitwiseXor { .. }
            | OperationKind::WrappingIntegerShiftLeft { .. }
            | OperationKind::WrappingIntegerShiftRight { .. }
            | OperationKind::ExactIntegerShiftLeft { .. }
            | OperationKind::ExactIntegerShiftRight { .. }
            | OperationKind::ExactIntegerAdd { .. }
            | OperationKind::ExactIntegerSubtract { .. }
            | OperationKind::ExactIntegerMultiply { .. }
            | OperationKind::ExactIntegerDivide { .. }
            | OperationKind::ExactIntegerRemainder { .. }
            | OperationKind::WrappingIntegerDivide { .. }
            | OperationKind::WrappingIntegerRemainder { .. }
            | OperationKind::SaturatingIntegerDivide { .. }
            | OperationKind::SaturatingIntegerRemainder { .. }
            | OperationKind::WrappingIntegerAdd { .. }
            | OperationKind::SaturatingIntegerAdd { .. }
            | OperationKind::WrappingIntegerSubtract { .. }
            | OperationKind::SaturatingIntegerSubtract { .. }
            | OperationKind::WrappingIntegerMultiply { .. }
            | OperationKind::SaturatingIntegerMultiply { .. }
    )
}
