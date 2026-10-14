use super::super::{
    AbstractOperation, BTreeSet, OwnershipEvent, PlaceId, StructuralPlaceDeclaration,
    StructuralPlaceKind,
};
use abstract_operations::AbstractStructuralCallResult;

pub(super) fn collect_places(operation: &AbstractOperation, places: &mut BTreeSet<PlaceId>) {
    use AbstractOperation as O;
    match operation {
        O::EstablishByteSequenceConcatenation {
            result,
            left,
            right,
            ..
        } => {
            places.extend([result.place, left.source(), right.source()]);
        }
        O::EstablishStructuralArray {
            result, elements, ..
        } => {
            places.insert(result.place);
            places.extend(elements.iter().map(|argument| argument.place));
        }
        abstract_operations::establish_record_or_case_pattern!(result, fields, ..) => {
            places.insert(result.place);
            places.extend(
                fields
                    .iter()
                    .filter_map(|initializer| match &initializer.value {
                        terminal_psi::RecordFieldValue::Structural(argument) => {
                            Some(argument.place)
                        }
                        terminal_psi::RecordFieldValue::Scalar { .. } => None,
                    }),
            );
        }
        O::Jump {
            structural_bindings,
            ..
        } => {
            places.extend(
                structural_bindings
                    .iter()
                    .flat_map(|binding| [binding.parameter, binding.argument.place]),
            );
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            places.extend(
                when_true
                    .structural_bindings
                    .iter()
                    .chain(&when_false.structural_bindings)
                    .flat_map(|binding| [binding.parameter, binding.argument.place]),
            );
        }
        O::ByteSequenceSubslice { source, result, .. } => {
            places.insert(source.place);
            places.insert(result.place);
        }
        O::ElementViewSubslice { source, result, .. } => {
            places.insert(source.place);
            places.insert(result.place);
        }
        O::EstablishElementView { result, source, .. } => {
            places.insert(result.place);
            places.insert(source.place);
        }
        O::PrimitiveLocalStore { destination, .. } => {
            places.insert(*destination);
        }
        O::WriteOnlyPrimitiveStore { destination, .. } => {
            places.insert(destination.place());
        }
        O::StructuralScalarFieldStore { destination, .. } => {
            places.insert(destination.place());
        }
        abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => {
            places.insert(place.id);
        }
        O::EstablishPrimitiveLocal { result, .. }
        | O::EstablishZeroedStructural { result, .. }
        | O::EstablishStructuralLocal { result, .. }
        | O::EstablishScalarArray { result, .. }
        | O::CallStructural {
            result: AbstractStructuralCallResult::Structural { result, .. },
            ..
        }
        | O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } => {
            places.insert(result.place);
        }
        O::StructuralLeafCopy { source, result, .. } => {
            places.insert(*source);
            places.insert(result.place);
        }
        O::EstablishReference { result, source, .. }
        | O::EstablishPlacedReference { result, source, .. } => {
            places.insert(result.place);
            places.insert(source.place);
        }
        O::ElementViewLength { source, .. } | O::ByteSequenceLength { source, .. } => {
            places.insert(source.place);
        }
        O::ReleaseReference { source, .. } => {
            places.insert(*source);
        }
        O::MoveStructuralField { source, result, .. } => {
            places.insert(source.place());
            places.insert(result.place);
        }
        O::StoreStructuralField {
            destination, value, ..
        } => {
            places.insert(destination.place());
            places.insert(value.place);
        }
        abstract_operations::byte_sequence_store_pair_pattern!(destination, source, ..) => {
            places.insert(*destination);
            places.insert(*source);
        }
        abstract_operations::scalar_read_or_membership_pattern!(source, ..)
        | O::ByteSequenceRead { source, .. }
        | abstract_operations::byte_sequence_write_pair_pattern!(destination: source, ..)
        | O::StructuralCase { source, .. }
        | O::ElementViewRead { source, .. }
        | abstract_operations::byte_len_or_field_read_pattern!(source, ..)
        | O::StructuralScalarFieldRead { source, .. }
        | O::ReturnStructural { source, .. } => {
            places.insert(*source);
        }
        O::AtomicEvent { event, .. } => {
            // The accessed atomic location joins place custody; a fence
            // accesses no place. A single-attempt compare-exchange also
            // establishes its structural outcome place.
            if let Some(place) = event.place() {
                places.insert(place);
            }
            if let abstract_operations::AbstractAtomicEvent::CompareExchangeOnce {
                outcome, ..
            } = event
            {
                places.insert(outcome.place);
            }
        }
        _ => {}
    }
}

pub(super) fn collect_operation_structural_places(
    operation: &AbstractOperation,
    structural_places: &mut Vec<StructuralPlaceDeclaration>,
) {
    match operation {
        abstract_operations::establish_or_subslice_set_pattern!(psi_operation,result, ..)
        | abstract_operations::establish_record_or_case_pattern!(psi_operation,
            result, ..)
        | AbstractOperation::EstablishZeroedStructural { psi_operation, result }
        | AbstractOperation::EstablishStructuralLocal { psi_operation, result }
        | AbstractOperation::EstablishReference {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishPlacedReference { psi_operation, result, .. }
        // The extraction's moved subtree is a fresh structural operation
        // result; its place joins the catalog exactly like an
        // establishment's.
        | AbstractOperation::MoveStructuralField {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Structural { result, .. },
            ..
        }
        | AbstractOperation::StructuralLeafCopy {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::BoundaryCall {
            psi_operation,
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } => structural_places.push(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: *psi_operation,
                structural_type: result.structural_type,
            },
        }),
        abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => {
            structural_places.push(*place);
        }
        AbstractOperation::AtomicEvent {
            psi_operation,
            event: abstract_operations::AbstractAtomicEvent::CompareExchangeOnce { outcome, .. },
            ..
        } => structural_places.push(StructuralPlaceDeclaration {
            id: outcome.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: *psi_operation,
                structural_type: outcome.structural_type,
            },
        }),
        _ => {}
    }
}

pub(super) fn operation_ownership(operation: &AbstractOperation) -> Vec<OwnershipEvent> {
    use AbstractOperation as O;
    match operation {
        O::CallStructural {
            claim_transfers, ..
        } => vec![OwnershipEvent::ClaimTransfer(
            claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect(),
        )],
        O::BoundaryCall {
            completion_receipts,
            ..
        } => vec![OwnershipEvent::ClaimCompletion(completion_receipts.clone())],
        abstract_operations::return_or_return_unit_pattern!(cleanup_actions, ..) => {
            vec![OwnershipEvent::Cleanup(cleanup_actions.clone())]
        }
        O::ReturnStructural {
            returned_claims,
            cleanup_actions,
            ..
        } => {
            vec![
                OwnershipEvent::StructuralReturn(
                    returned_claims
                        .iter()
                        .map(|binding| binding.claim)
                        .collect(),
                ),
                OwnershipEvent::Cleanup(cleanup_actions.clone()),
            ]
        }
        O::ExternalCompletion {
            frontier_lower_bound,
            ..
        } => {
            vec![OwnershipEvent::ExternalCompletionFrontier(
                frontier_lower_bound.clone(),
            )]
        }
        O::Crash {
            frontier_lower_bound,
            ..
        } => {
            vec![OwnershipEvent::CrashFrontier(frontier_lower_bound.clone())]
        }
        _ => Vec::new(),
    }
}
