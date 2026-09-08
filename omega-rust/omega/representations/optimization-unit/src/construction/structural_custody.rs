use super::*;

pub(super) fn collect_places(operation: &AbstractOperation, places: &mut BTreeSet<PlaceId>) {
    use AbstractOperation as O;
    match operation {
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
        O::PrimitiveLocalStore { destination, .. } => {
            places.insert(*destination);
        }
        O::WriteOnlyPrimitiveStore { destination, .. }
        | O::StructuralScalarFieldStore { destination, .. } => {
            places.insert(destination.place);
        }
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => {
            places.insert(place.id);
        }
        O::EstablishPrimitiveLocal { result, .. }
        | O::EstablishPayloadlessCase { result, .. }
        | O::EstablishAffineScalarRecord { result, .. }
        | O::CallStructural { result, .. }
        | O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } => {
            places.insert(result.place);
        }
        O::PrimitiveScalarRead { source, .. }
        | O::ByteSequenceRead { source, .. }
        | O::StructuralCase { source, .. }
        | O::ByteSequenceLength { source, .. }
        | O::BooleanStructuralField { source, .. }
        | O::ReturnStructural { source, .. } => {
            places.insert(*source);
        }
        O::IntegerStructuralField { source, .. } => {
            places.insert(source.place);
        }
        O::StoreDynamicDescriptor { stored, .. } => {
            places.insert(stored.selection.source.place);
        }
        O::CallStoredDynamicScalar {
            dynamic_dispatch, ..
        } => {
            places.insert(dynamic_dispatch.stored.selection.source.place);
        }
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        }
        | O::CallDynamicUnit {
            dynamic_dispatch, ..
        } => {
            places.insert(dynamic_dispatch.initial.source.place);
            places.insert(dynamic_dispatch.rebound.source.place);
        }
        O::CallStructuralScalarWithDynamicArguments {
            structural_arguments,
            dynamic_arguments,
            ..
        }
        | O::CallUnitWithDynamicArguments {
            structural_arguments,
            dynamic_arguments,
            ..
        } => {
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
        _ => {}
    }
}

pub(super) fn collect_operation_structural_places(
    operation: &AbstractOperation,
    structural_places: &mut Vec<StructuralPlaceDeclaration>,
) {
    match operation {
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ByteSequenceSubslice {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishPayloadlessCase {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::EstablishAffineScalarRecord {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::CallStructural {
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
        _ => {}
    }
}

pub(super) fn operation_ownership(operation: &AbstractOperation) -> Vec<OwnershipEvent> {
    use AbstractOperation as O;
    match operation {
        O::CallUnit {
            claim_transfers, ..
        }
        | O::CallUnitWithDynamicArguments {
            claim_transfers, ..
        }
        | O::CallStructuralScalar {
            claim_transfers, ..
        } => {
            vec![OwnershipEvent::ClaimTransfer(
                claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect(),
            )]
        }
        O::CallStructuralScalarWithDynamicArguments {
            claim_transfers, ..
        } => vec![OwnershipEvent::ClaimTransfer(
            claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect(),
        )],
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
