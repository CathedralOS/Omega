//! Owned aggregate actuals rejoin their exact incoming storage or dominating producer.
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::target;

use super::{
    AbstractOperation, AbstractOperationPlan, CallPlan, PsiOptimizationFunction,
    StructuralMultiplicity, StructuralPlaceKind, TargetOperationPlan,
};
use crate::legalization::scalar_graph_input::structural_case;
use crate::legalization::scalar_graph_input::structural_parameters;

pub(super) fn reconstruct(
    argument: &terminal_psi::StructuralArgument,
    position: usize,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    call: &CallPlan,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    let destination = callee
        .structural_parameters
        .get(position)
        .ok_or(LegalizationError::custody())?;
    // Whole-root domain qualifications are signature preconditions whose
    // discharge was verified at the call edge upstream; admission binds the
    // roster and re-checks it exactly wherever an argument binds to a
    // parameter. Projected (path-beneath-root) qualifications still decline.
    if !argument.path.is_empty()
        || destination.access != terminal_psi::StructuralAccess::Owned
        || destination.multiplicity == StructuralMultiplicity::Linear
        || destination.is_self
        || !destination.projected_qualifications.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    let shape = crate::structural_inputs::structural_reference_input::parameter_shape(
        destination,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
    let value_shape = crate::structural_inputs::structural_reference_input::owned_aggregate_shape(
        destination.structural_type,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
    if shape != value_shape {
        return Err(LegalizationError::custody());
    }
    let source = if let Some(parameter) = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
    {
        if parameter.structural_type != destination.structural_type
            || parameter.access != destination.access
            || parameter.multiplicity != destination.multiplicity
            || parameter.is_self
            || parameter.qualifications != destination.qualifications
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        let retained = native
            .functions
            .iter()
            .find(|function| function.machine == caller.machine)
            .and_then(structural_parameters)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|retained| retained.place == argument.place)
            })
            .ok_or(LegalizationError::custody())?;
        if retained.structural_type != parameter.structural_type
            || retained.access != parameter.access
            || retained.multiplicity != parameter.multiplicity
            || retained.shape != shape
            || retained.placement.shape != shape
            || !retained.projected_qualifications.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        retained.placement.clone().into()
    } else if let Some((producer, structural_type, producer_site)) =
        trivial_affine_local(caller, argument.place)
    {
        // An empty-record local has no bytes: the call replays its exact
        // establishment as the producer, and the zero-byte placement carries
        // nothing. The establishment must precede the call like any producer.
        if structural_type != destination.structural_type
            || destination.multiplicity != StructuralMultiplicity::Affine
            || !destination.qualifications.is_empty()
            || shape.byte_size != 0
            || !precedes(caller, producer_site, call_operation, argument)?
        {
            return Err(LegalizationError::custody());
        }
        target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        }
    } else {
        let (producer, result) = structural_case::source_result(caller, argument.place)?;
        if result.structural_type != destination.structural_type
            || result.multiplicity != destination.multiplicity
            || !result.claims.is_empty()
            || result.qualifications != destination.qualifications
            || !result.projected_qualifications.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        let producer_site = node_site(caller, |operation| {
            matches!(operation,
                AbstractOperation::EstablishScalarArray { psi_operation, result: actual, .. }
                | AbstractOperation::EstablishRecord { psi_operation, result: actual, .. }
                | AbstractOperation::EstablishScalarCase { psi_operation, result: actual, .. }
                | AbstractOperation::CallStructural { psi_operation, result: actual, .. }
                if *psi_operation == producer && actual == result)
        })
        .ok_or(LegalizationError::custody())?;
        if !precedes(caller, producer_site, call_operation, argument)?
            || !caller.structural_places.iter().any(|place| {
                place.id == result.place
                    && place.kind
                        == StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        }
            })
        {
            return Err(LegalizationError::custody());
        }
        target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        }
    };
    let placement = call
        .parameters
        .get(callee.parameters.len() + position)
        .ok_or(LegalizationError::custody())?;
    if placement.shape != shape {
        return Err(LegalizationError::custody());
    }
    Ok(target_operations::TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: Vec::new(),
        root_structural_type: destination.structural_type,
        structural_type: destination.structural_type,
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: placement.clone(),
    })
}

/// The block and position of the one node `matches` selects.
fn node_site(
    caller: &PsiOptimizationFunction,
    matches: impl Fn(&AbstractOperation) -> bool,
) -> Option<(semantic_vocabulary::BlockId, usize)> {
    caller.blocks.iter().find_map(|block| {
        block
            .nodes
            .iter()
            .position(|node| matches(&node.operation))
            .map(|position| (block.id, position))
    })
}

/// Whether the producer at `producer_site` runs before the call that passes
/// `argument`: earlier in the call's block, or in a dominating block.
fn precedes(
    caller: &PsiOptimizationFunction,
    producer_site: (semantic_vocabulary::BlockId, usize),
    call_operation: semantic_vocabulary::OperationId,
    argument: &terminal_psi::StructuralArgument,
) -> Result<bool, LegalizationError> {
    let call_site = node_site(caller, |operation| match operation {
        AbstractOperation::CallStructural {
            psi_operation,
            structural_arguments,
            ..
        }
        | AbstractOperation::CallUnit {
            psi_operation,
            structural_arguments,
            ..
        }
        | AbstractOperation::CallStructuralScalar {
            psi_operation,
            structural_arguments,
            ..
        } => *psi_operation == call_operation && structural_arguments.contains(argument),
        _ => false,
    })
    .ok_or(LegalizationError::custody())?;
    Ok(if producer_site.0 == call_site.0 {
        producer_site.1 < call_site.1
    } else {
        target::control_flow::sources::dominates(caller, producer_site.0, call_site.0)
    })
}

/// The one `EstablishTrivialAffineLocal` minting `place`: its operation,
/// declared empty-record type and site.
fn trivial_affine_local(
    caller: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<(
    semantic_vocabulary::OperationId,
    semantic_vocabulary::StructuralTypeId,
    (semantic_vocabulary::BlockId, usize),
)> {
    let mut establishments = caller.blocks.iter().flat_map(|block| {
        block
            .nodes
            .iter()
            .enumerate()
            .filter_map(move |(position, node)| match &node.operation {
                AbstractOperation::EstablishTrivialAffineLocal {
                    psi_operation,
                    place: declaration,
                    structural_type,
                } if declaration.id == place => {
                    Some((*psi_operation, structural_type.id, (block.id, position)))
                }
                _ => None,
            })
    });
    let establishment = establishments.next()?;
    establishments.next().is_none().then_some(establishment)
}
