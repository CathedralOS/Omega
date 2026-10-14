//! Declared-place reconstruction and claim/place reference validation.
use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{ClaimId, MachineId, PlaceId};
use std::collections::BTreeSet;

/// Invocation, block-binding and result claims share the identity namespace.
/// Membership does not establish a birth, a current home, or liveness: structural
/// foundation and current-ownership replay independently authenticate those.
/// `entry_claims` remains the independently checked ordinary-entry index;
/// content and operation claims retain their complete source declarations.
pub(crate) fn function_has_claim(function: &PsiOptimizationFunction, claim: ClaimId) -> bool {
    function.entry_claims.contains(&claim)
        || function
            .block_entry_claims
            .iter()
            .any(|binding| binding.claim == claim)
        || function
            .content_entry_claims
            .iter()
            .any(|candidate| candidate.claim == claim)
        || function
            .block_nodes()
            .filter_map(|node| node.operation.structural_result())
            .any(|(_, result)| result.claims.iter().any(|binding| binding.claim == claim))
}

pub(crate) fn reconstruct_declared_places(
    function: &PsiOptimizationFunction,
) -> Result<BTreeSet<PlaceId>, OptimizationUnitValidationError> {
    let mut known_places = function
        .structural_parameters
        .iter()
        .chain(function.block_structural_parameters())
        .map(|parameter| parameter.place)
        .chain(
            function
                .entry_claim_declarations
                .iter()
                .map(|claim| claim.input),
        )
        .chain(function.result.structural().map(|result| result.place))
        .collect::<BTreeSet<_>>();
    for node in function.block_nodes() {
        match &node.operation {
            abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => {
                known_places.insert(place.id);
            }
            abstract_operations::establish_or_subslice_set_pattern!(result, ..)
            | abstract_operations::establish_case_or_record_result_pattern!(result)
            | O::EstablishZeroedStructural { result, .. }
            | O::EstablishStructuralLocal { result, .. }
            | O::EstablishReference { result, .. }
            | O::EstablishPlacedReference { result, .. }
            // The extraction's moved subtree is a declared operation
            // result place like any establishment's.
            | O::MoveStructuralField { result, .. }
            // The copy's fresh owned storage is a declared operation
            // result place like any establishment's.
            | O::StructuralLeafCopy { result, .. }
            | O::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            }
            | O::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } => {
                known_places.insert(result.place);
            }
            O::AtomicEvent { event, .. } => {
                // The accessed atomic location joins place custody; a
                // fence accesses no place. A single-attempt
                // compare-exchange also establishes its structural
                // outcome place.
                if let Some(place) = event.place() {
                    known_places.insert(place);
                }
                if let abstract_operations::AbstractAtomicEvent::CompareExchangeOnce {
                    outcome,
                    ..
                } = event
                {
                    known_places.insert(outcome.place);
                }
            }
            _ => {}
        }
    }
    for node in function.block_nodes() {
        validate_operation_places(function.machine, &node.operation, &known_places)?;
    }
    Ok(known_places)
}

pub(crate) fn validate_operation_places(
    machine: MachineId,
    operation: &abstract_operations::AbstractOperation,
    known: &BTreeSet<PlaceId>,
) -> Result<(), OptimizationUnitValidationError> {
    use abstract_operations::AbstractOperation as O;
    let require = |place: PlaceId, known: &BTreeSet<PlaceId>| {
        if known.contains(&place) {
            Ok(())
        } else {
            Err(OptimizationUnitValidationError::UnknownPlace { machine, place })
        }
    };
    for (_, actions) in operation.cleanup_rosters() {
        for action in actions {
            require(action.place(), known)?;
        }
    }
    match operation {
        O::EstablishByteSequenceConcatenation { left, right, .. } => {
            require(left.source(), known)?;
            require(right.source(), known)?;
        }
        O::EstablishStructuralArray { elements, .. } => {
            for argument in elements {
                require(argument.place, known)?;
            }
        }
        abstract_operations::establish_case_or_record_fields_pattern!(fields) => {
            for initializer in fields {
                if let terminal_psi::RecordFieldValue::Structural(argument) = &initializer.value {
                    require(argument.place, known)?;
                }
            }
        }
        O::StructuralCase { source, .. } => {
            require(*source, known)?;
        }
        O::Jump {
            structural_bindings,
            ..
        } => {
            for binding in structural_bindings {
                require(binding.parameter, known)?;
                require(binding.argument.place, known)?;
            }
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            for binding in when_true
                .structural_bindings
                .iter()
                .chain(&when_false.structural_bindings)
            {
                require(binding.parameter, known)?;
                require(binding.argument.place, known)?;
            }
        }
        abstract_operations::byte_literal_establish_pair_pattern!(..) => {}
        O::PrimitiveLocalStore { destination, .. } => require(*destination, known)?,
        O::EstablishReference { source, .. } | O::EstablishPlacedReference { source, .. } => {
            require(source.place, known)?
        }
        O::ReleaseReference { source, .. } => require(*source, known)?,
        O::WriteOnlyPrimitiveStore { destination, .. } => {
            require(destination.place(), known)?;
        }
        O::StructuralScalarFieldStore { destination, .. } => {
            require(destination.place(), known)?;
        }
        O::MoveStructuralField { source, .. } => {
            require(source.place(), known)?;
        }
        O::StructuralLeafCopy { source, .. } => {
            require(*source, known)?;
        }
        O::StoreStructuralField {
            destination, value, ..
        } => {
            require(destination.place(), known)?;
            require(value.place, known)?;
        }
        abstract_operations::boundary_or_structural_call_pair_pattern!(
            structural_arguments,
            ..
        ) => {
            for argument in structural_arguments {
                require(argument.place, known)?;
            }
        }
        abstract_operations::byte_sequence_store_pair_pattern!(destination, source, ..) => {
            require(*destination, known)?;
            require(*source, known)?;
        }
        abstract_operations::scalar_read_or_membership_pattern!(source, ..)
        | O::ByteSequenceRead { source, .. }
        | abstract_operations::byte_sequence_write_pair_pattern!(destination: source, ..)
        | abstract_operations::byte_len_or_field_read_pattern!(source, ..)
        | O::StructuralScalarFieldRead { source, .. } => {
            require(*source, known)?;
        }
        O::ByteSequenceLength { source, .. }
        | O::ElementViewLength { source, .. }
        | O::ByteSequenceSubslice { source, .. }
        | O::ElementViewSubslice { source, .. } => require(source.place, known)?,
        O::ReturnStructural { source, .. } => require(*source, known)?,
        _ => {}
    }
    Ok(())
}
