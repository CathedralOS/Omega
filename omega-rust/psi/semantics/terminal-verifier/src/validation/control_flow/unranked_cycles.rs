//! Eligibility for cyclic scalar work, owned inputs, locals, views, and receivers.

use super::super::{block_views, byte_sequence_subslice, primitive_storage};
use super::*;

/// Eligibility carries no proof or dominance authority. The caller runs the
/// ordinary operand, view, successor, and frontier checks after this fence.
pub(super) fn eligible(module: &TerminalModule, machine: &TerminalMachine) -> bool {
    let scalar_case_result = machine.result.structural().is_some_and(|result| {
        matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        ) && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && super::super::scalar_case::plain_type(module, result.structural_type)
    });
    if machine
        .ranked_scc
        .as_ref()
        .is_some_and(|ranking| ranking.as_unsigned_countdown().is_some())
        || (machine.result.structural().is_some() && !scalar_case_result)
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
            !plain_owned(parameter)
                && (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !(persistent_receiver(module, parameter)
                        || ((parameter.access == StructuralAccess::SharedBorrow
                            || (parameter.access == StructuralAccess::MutableBorrow
                                && (machine.result == TerminalMachineResult::Unit
                                    || scalar_case_result)))
                            && module.structural_types.iter().any(|declaration| {
                                declaration.id == parameter.structural_type
                                    && matches!(
                                        declaration.shape,
                                        StructuralTypeShape::ByteSequence(
                                            terminal_psi::ByteSequenceCarrier::BorrowedView
                                        )
                                    )
                            }))))
        })
        || machine.structural_places.iter().any(|place| {
            !(matches!(
                place.kind,
                StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::BlockParameter { .. }
                    | StructuralPlaceKind::OperationResult { .. }
                    | StructuralPlaceKind::ByteSequenceLiteral { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
            ) || scalar_case_result && place.kind == StructuralPlaceKind::Result)
        })
    {
        return false;
    }
    machine.blocks.iter().all(|block| {
        let case_source = local_case_result(module, block);
        let terminator_eligible =
            matches!(
                block.terminator,
                Terminator::Jump { .. }
                    | Terminator::Conditional { .. }
                    | Terminator::Return { .. }
                    | Terminator::ReturnUnit { .. }
                    | Terminator::Crash { .. }
            ) || matches!(block.terminator, Terminator::StructuralCase { .. })
                && case_source.is_some()
                || scalar_case_result && matches!(&block.terminator,
                    Terminator::ReturnStructural { source, returned_claims, .. }
                    if returned_claims.is_empty()
                        && super::super::scalar_case::plain_return_source(module, machine, *source));
        terminator_eligible
            && block
                .operations
                .iter()
                .all(|operation| match &operation.kind {
                    OperationKind::EstablishScalarCase { .. } => {
                        super::super::scalar_case::fields(module, machine, operation).is_ok()
                            && operation.result.structural().is_some_and(|result|
                                matches!(&block.terminator, Terminator::ReturnStructural { source, .. }
                                    if *source == result.place) || case_source == Some(result.place))
                    }
                    OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. } => {
                        operation.result.structural().is_some_and(|result|
                            case_source == Some(result.place)
                                || matches!(&block.terminator, Terminator::ReturnStructural { source, .. }
                                    if *source == result.place))
                    }
                    // Ordinary scalar calls retain their complete signature,
                    // requirement, and crash checks after this eligibility fence.
                    OperationKind::Call { .. } => operation.result.scalar().is_some(),
                    OperationKind::PortWrite { .. } => operation.result == OperationResult::Unit,
                    OperationKind::EstablishPrimitiveLocal { .. } => {
                        primitive_storage::validate_establishment(module, machine, operation)
                            .is_ok()
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
                                    && ((argument.access != StructuralAccess::Owned
                                        && primitive_storage::local_result(
                                            machine,
                                            argument.place,
                                        )
                                        .is_some())
                                        || owned_argument(machine, argument))
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
                        (operation.result == OperationResult::Unit
                            || operation
                                .result
                                .structural()
                                .is_some_and(|result| case_source == Some(result.place)))
                            && completion_receipts.is_empty()
                            && structural_arguments.iter().all(|argument| {
                                argument.path.is_empty()
                                    && argument.access == StructuralAccess::SharedBorrow
                                    && (machine
                                        .structural_parameters
                                        .iter()
                                        .any(|parameter| parameter.place == argument.place)
                                        || block_views::parameter(machine, argument.place)
                                            .is_some()
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
                                            .is_ok())
                                        || owned_argument(machine, argument))
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
                    | OperationKind::ByteSequenceWrite { .. }
                    | OperationKind::StructuralByteSequenceFieldStore { .. }
                    | OperationKind::StructuralByteSequenceFieldByteStore { .. }
                    | OperationKind::EstablishByteSequenceLiteral { .. } => {
                        operation.result == OperationResult::Unit
                    }
                    kind => operation.result.scalar().is_some() && pure_scalar(kind),
                })
    })
}

/// The owned sum never becomes loop-carried custody. The same block establishes
/// and inspects it, and every selected edge disposes that exact whole root.
/// Ordinary operation formation, dominance and frontier replay remain required.
fn local_case_result(
    module: &TerminalModule,
    block: &terminal_psi::Block,
) -> Option<semantic_vocabulary::PlaceId> {
    let Terminator::StructuralCase { source, cases } = &block.terminator else {
        return None;
    };
    if cases.is_empty()
        || cases
            .iter()
            .any(|case| case.trivial_affine_discards != [*source])
    {
        return None;
    }
    let result = block.operations.iter().find_map(|operation| {
        matches!(
            operation.kind,
            OperationKind::BoundaryCall { .. }
                | OperationKind::EstablishScalarCase { .. }
                | OperationKind::CallStructural { .. }
                | OperationKind::CallStructuralWithScalarArguments { .. }
        )
        .then(|| operation.result.structural())
        .flatten()
        .filter(|result| result.place == *source)
    })?;
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return None;
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    cases
        .iter()
        .all(|case| {
            case.fields.iter().all(|field| {
                !field.relevance.is_erased()
                    && matches!(
                        field.field_type,
                        terminal_psi::StructuralFieldType::Scalar(_)
                            | terminal_psi::StructuralFieldType::BoundedInteger(_)
                    )
            })
        })
        .then_some(*source)
}

fn plain_owned(parameter: &StructuralParameterDeclaration) -> bool {
    !parameter.is_self
        && parameter.access == StructuralAccess::Owned
        && matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
}

fn owned_argument(machine: &TerminalMachine, argument: &StructuralArgument) -> bool {
    argument.access == StructuralAccess::Owned
        && argument.path.is_empty()
        && machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .or_else(|| block_views::parameter(machine, argument.place))
            .is_some_and(plain_owned)
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
            | OperationKind::IeeeFloatCompare { .. }
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
