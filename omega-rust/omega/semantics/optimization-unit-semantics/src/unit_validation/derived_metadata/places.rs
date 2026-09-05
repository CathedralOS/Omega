//! Declared-place reconstruction and claim/place reference validation.

use super::*;

/// Terminal ownership treats ordinary and content entry claims as one live
/// claim namespace while retaining their declarations as distinct authority.
/// `entry_claims` remains the independently checked ordinary-claim index;
/// content-only claims are resolved from their complete retained catalog.
pub(crate) fn function_has_claim(function: &PsiOptimizationFunction, claim: ClaimId) -> bool {
    function.entry_claims.contains(&claim)
        || function
            .content_entry_claims
            .iter()
            .any(|candidate| candidate.claim == claim)
}

pub(crate) fn reconstruct_declared_places(
    function: &PsiOptimizationFunction,
) -> Result<BTreeSet<PlaceId>, OptimizationUnitValidationError> {
    let mut known_places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(
            function
                .entry_claim_declarations
                .iter()
                .map(|claim| claim.input),
        )
        .chain(function.result.structural().map(|result| result.place))
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        for node in &block.nodes {
            match &node.operation {
                O::EstablishByteSequenceLiteral { place, .. }
                | O::EstablishTrivialAffineLocal { place, .. } => {
                    known_places.insert(place.id);
                }
                O::EstablishPayloadlessCase { result, .. }
                | O::EstablishAffineScalarRecord { result, .. }
                | O::CallStructural { result, .. } => {
                    known_places.insert(result.place);
                }
                _ => {}
            }
        }
    }
    for block in &function.blocks {
        for node in &block.nodes {
            validate_operation_places(function.machine, &node.operation, &known_places)?;
        }
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
    match operation {
        O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishAffineScalarRecord { .. } => {}
        O::WriteOnlyPrimitiveStore { destination, .. }
        | O::StructuralScalarFieldStore { destination, .. } => {
            require(destination.place, known)?;
        }
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                require(argument.place, known)?;
            }
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
            for argument in structural_arguments {
                require(argument.place, known)?;
            }
            for argument in dynamic_arguments {
                match &argument.source {
                    abstract_operations::AbstractDynamicDescriptorSource::Selection {
                        selection,
                        ..
                    } => require(selection.source.place, known)?,
                    abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                        initial,
                        rebound,
                        ..
                    } => {
                        require(initial.source.place, known)?;
                        require(rebound.source.place, known)?;
                    }
                    abstract_operations::AbstractDynamicDescriptorSource::Parameter(_) => {}
                }
            }
        }
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            require(*source, known)?;
        }
        O::IntegerStructuralField { source, .. } => require(source.place, known)?,
        O::StoreDynamicDescriptor { stored, .. } => {
            require(stored.selection.source.place, known)?;
        }
        O::CallStoredDynamicScalar {
            dynamic_dispatch, ..
        } => {
            require(dynamic_dispatch.stored.selection.source.place, known)?;
        }
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        }
        | O::CallDynamicUnit {
            dynamic_dispatch, ..
        } => {
            require(dynamic_dispatch.initial.source.place, known)?;
            require(dynamic_dispatch.rebound.source.place, known)?;
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            for cleanup in cleanup_actions {
                let place = match cleanup {
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        discard.place
                    }
                    terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        cleanup.place
                    }
                };
                require(place, known)?;
            }
        }
        _ => {}
    }
    Ok(())
}
