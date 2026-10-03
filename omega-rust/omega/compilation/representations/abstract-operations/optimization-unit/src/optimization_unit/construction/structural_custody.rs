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
        O::EstablishCase { result, fields, .. } | O::EstablishRecord { result, fields, .. } => {
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
            places.insert(*source);
            places.insert(result.place);
        }
        O::ElementViewSubslice { source, result, .. } => {
            places.insert(*source);
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
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => {
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
        O::EstablishReference { result, source, .. } => {
            places.insert(result.place);
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
        O::ByteSequenceStore {
            destination,
            source,
            ..
        }
        | O::StructuralByteSequenceFieldStore {
            destination,
            source,
            ..
        } => {
            places.insert(*destination);
            places.insert(*source);
        }
        O::PrimitiveScalarRead { source, .. }
        | O::StructuralCaseMembership { source, .. }
        | O::ByteSequenceRead { source, .. }
        | O::ByteSequenceWrite {
            destination: source,
            ..
        }
        | O::StructuralByteSequenceFieldByteStore {
            destination: source,
            ..
        }
        | O::StructuralCase { source, .. }
        | O::ByteSequenceLength { source, .. }
        | O::ElementViewLength { source, .. }
        | O::ElementViewRead { source, .. }
        | O::StructuralByteSequenceFieldLength { source, .. }
        | O::StructuralByteSequenceFieldRead { source, .. }
        | O::StructuralScalarFieldRead { source, .. }
        | O::ReturnStructural { source, .. } => {
            places.insert(*source);
        }
        O::StoreDynamicDescriptor { stored, .. } => {
            places.insert(stored.selection.source.place);
        }
        O::CallDynamic { dispatch, .. } => match dispatch {
            abstract_operations::AbstractDynamicDispatch::Stored(dispatch) => {
                places.insert(dispatch.stored.selection.source.place);
            }
            abstract_operations::AbstractDynamicDispatch::Rebound(dispatch) => {
                places.insert(dispatch.initial.source.place);
                places.insert(dispatch.rebound.source.place);
            }
            // A received descriptor names no caller-local source place.
            abstract_operations::AbstractDynamicDispatch::Parameter(_) => {}
        },
        // Only a descriptor-forwarding call declares its argument places
        // here; a plain call adds none.
        O::CallStructural {
            structural_arguments,
            dynamic_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } if !dynamic_arguments.is_empty() => {
            places.extend(structural_arguments.iter().map(|argument| argument.place));
            for argument in dynamic_arguments {
                match &argument.source {
                    abstract_operations::AbstractDynamicDescriptorSource::Selection {
                        selection,
                        ..
                    } => {
                        places.insert(selection.source.place);
                    }
                    abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                        initial,
                        rebound,
                        ..
                    } => {
                        places.insert(initial.source.place);
                        places.insert(rebound.source.place);
                    }
                    abstract_operations::AbstractDynamicDescriptorSource::Parameter(_) => {}
                }
            }
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
        AbstractOperation::EstablishByteSequenceConcatenation { psi_operation, result, .. }
            | AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ByteSequenceSubslice {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishElementView {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ElementViewSubslice {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishScalarArray {
            psi_operation,
            result,
            ..
        } | AbstractOperation::EstablishStructuralArray {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishCase {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishRecord {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishZeroedStructural { psi_operation, result }
        | AbstractOperation::EstablishStructuralLocal { psi_operation, result }
        | AbstractOperation::EstablishReference {
            psi_operation,
            result,
            ..
        }
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
        AbstractOperation::EstablishByteSequenceLiteral { place, .. }
        | AbstractOperation::EstablishTrivialAffineLocal { place, .. } => {
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
        } => vec![OwnershipEvent::ClaimCompletion(
            completion_receipts
                .iter()
                .map(|receipt| receipt.claim)
                .collect(),
        )],
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            vec![OwnershipEvent::Cleanup(cleanup_actions.clone())]
        }
        O::ReturnStructural {
            returned_claims, ..
        } => {
            vec![OwnershipEvent::StructuralReturn(returned_claims.clone())]
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
