//! Evaluated-constant in-place installation shared by every constant
//! replacement family.
//!
//! The integer- and boolean-evaluation candidates, the proof-certified
//! same-operand laws, and the proof-certified unit-divisor laws all replay
//! the same acceptance: the patched node is rewritten to the computed
//! `*Constant` operation, its definition roster is reset to the single
//! produced scalar, uses/successors/ownership are cleared, the shared output
//! tail rebuilds the unit's derived surfaces and revalidates it, and the
//! replacement must keep the closed scalar observation and live boundary the
//! input spelled. Only the constant operation's construction and its
//! definition's scalar type differ per family.

use crate::OptimizationUnitValidationError;
use crate::candidates::observation::{
    ClosedScalarObservationBoundary, reconstruct_closed_scalar_node_boundary,
};
use crate::candidates::sparse_conditional_constant_propagation::{
    observation_at, same_closed_scalar_observation,
};
use optimization_unit::{
    NodeLocation, PsiNodeObservation, PsiOptimizationUnit, ValueDefinition, ValueDefinitionSite,
};
use semantic_vocabulary::{ScalarType, ValueId};

/// Rewrite `location`'s node to `operation`, rebuild the unit's derived
/// surfaces, and return the revalidated output — or the validation error the
/// rebuilt boundaries raise.
pub(crate) fn install_evaluated_constant(
    input: &PsiOptimizationUnit,
    location: NodeLocation,
    operation: abstract_operations::AbstractOperation,
    result: ValueId,
    scalar_type: ScalarType,
    input_observation: &PsiNodeObservation,
    input_live: &ClosedScalarObservationBoundary,
) -> Result<PsiOptimizationUnit, OptimizationUnitValidationError> {
    let mut output = input.clone();
    let function = output
        .function_mut(location.machine)
        .expect("candidate source function exists");
    let block = function
        .block_mut(location.block)
        .expect("candidate source block exists");
    let node = &mut block.nodes[usize::try_from(location.node).expect("u32 fits usize")];
    node.operation = operation;
    node.definitions = vec![ValueDefinition {
        value: result,
        scalar_type,
        site: ValueDefinitionSite::Node {
            block: location.block,
            node: location.node,
        },
    }];
    node.uses.clear();
    node.successors.clear();
    node.ownership.clear();
    crate::unit_validation::revalidate_rewrite_output(&mut output, location.machine)?;
    let output_observation = observation_at(&output, location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if !same_closed_scalar_observation(input_observation, &output_observation) {
        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
    }
    let output_live = reconstruct_closed_scalar_node_boundary(&output, location)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if input_live.live_out != output_live.live_out
        || output_live
            .live_in
            .iter()
            .any(|value| !input_live.live_in.contains(value))
    {
        return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
    }
    Ok(output)
}
