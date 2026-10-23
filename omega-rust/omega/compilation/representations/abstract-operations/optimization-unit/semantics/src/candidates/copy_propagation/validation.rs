//! Redundant-parameter witness reconstruction and rewrite acceptance.
use crate::candidates::{canonicalize_provenance, provenance_realized_at};
use crate::unit_validation::derived_metadata::{
    expected_definitions, expected_edges, expected_ownership, expected_uses,
};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_core::OptimizationValidatorIdentity;
use optimization_unit::{
    BlockParameterIncomingBinding, NodeLocation, PsiOptimizationUnit, PsiRealizationSite,
    PsiRewriteCandidate, PsiRewritePatch, ValueDefinitionSite,
    reconstruct_psi_closed_region_observation,
};
use std::collections::BTreeSet;

use super::observation::{
    normalize_redundant_parameter_observation_input, unchanged_outside_redundant_parameter_region,
};
use super::operation_rewrite::rewrite_block_parameter_operation;
use crate::candidates::{candidate_block, candidate_function};

pub(super) fn validate_redundant_block_parameter_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::RemoveRedundantBlockParameter(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let witness = candidate
        .redundant_block_parameter_witness()
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let function = candidate_function(input, patch.machine)?;
    if function.entry == patch.block || patch.parameter == patch.replacement {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    let block = candidate_block(function, patch.block)?;
    let position = usize::try_from(patch.position).expect("u32 fits usize");
    let Some(parameter) = block.parameters.get(position) else {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    };
    if parameter.value != patch.parameter
        || parameter.scalar_type != patch.scalar_type
        || parameter.site
            != (ValueDefinitionSite::BlockParameter {
                block: patch.block,
                position: patch.position,
            })
    {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    let replacement_type = function
        .parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter().chain(block.definitions())),
        )
        .find(|definition| definition.value == patch.replacement)
        .map(|definition| definition.scalar_type);
    if replacement_type != Some(patch.scalar_type) {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }

    let mut incoming = Vec::new();
    let mut expected_provenance = Vec::new();
    let mut affected_blocks = BTreeSet::from([patch.block]);
    for source in &function.blocks {
        for (node_index, node) in source.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: patch.machine,
                block: source.id,
                node: u32::try_from(node_index).expect("unit node index fits u32"),
            };
            let changes_use = node
                .uses
                .iter()
                .any(|use_site| use_site.value == patch.parameter);
            for edge in &node.successors {
                if edge.target != patch.block {
                    continue;
                }
                let Some(binding) = edge.bindings.get(position) else {
                    return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
                };
                incoming.push(BlockParameterIncomingBinding {
                    source: source.id,
                    edge: edge.psi_edge,
                    argument: binding.argument,
                });
                let site = PsiRealizationSite::Edge {
                    machine: patch.machine,
                    edge: edge.psi_edge,
                };
                expected_provenance.push(provenance_realized_at(
                    site,
                    site,
                    &edge.provenance,
                    &edge.fuel,
                ));
            }
            if changes_use {
                affected_blocks.insert(source.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    expected_provenance.push(provenance_realized_at(
                        site,
                        site,
                        &node.provenance,
                        &node.fuel,
                    ));
                }
            }
            if node
                .successors
                .iter()
                .any(|edge| edge.target == patch.block)
            {
                affected_blocks.insert(source.id);
            }
        }
    }
    incoming.sort_by_key(|row| (row.edge, row.source));
    canonicalize_provenance(&mut expected_provenance);
    if incoming != witness.incoming
        || incoming
            .iter()
            .any(|binding| binding.argument != patch.replacement)
    {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    if candidate.substitutions()
        != [optimization_unit::ScalarSubstitution {
            from: patch.parameter,
            to: patch.replacement,
            scalar_type: patch.scalar_type,
        }]
    {
        return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
    }
    if candidate.affected_blocks() != affected_blocks.into_iter().collect::<Vec<_>>()
        || candidate.provenance() != expected_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let normalized_input =
        normalize_redundant_parameter_observation_input(input, patch, candidate.affected_blocks())?;
    let input_region = reconstruct_psi_closed_region_observation(
        &normalized_input,
        patch.machine,
        candidate.affected_blocks(),
    )
    .ok_or(OptimizationUnitValidationError::CandidateRegionObservationUnavailable)?;

    let mut output = input.clone();
    let function = output
        .function_mut(patch.machine)
        .expect("candidate function exists");
    let block = function
        .block_mut(patch.block)
        .expect("candidate block exists");
    block.parameters.remove(position);
    for (new_position, parameter) in block.parameters.iter_mut().enumerate().skip(position) {
        parameter.site = ValueDefinitionSite::BlockParameter {
            block: patch.block,
            position: u32::try_from(new_position).expect("parameter index fits u32"),
        };
    }
    for block in &mut function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            rewrite_block_parameter_operation(&mut node.operation, patch);
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = expected_edges(&node.operation);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    crate::unit_validation::revalidate_rewrite_output(&mut output, patch.machine)?;
    if !unchanged_outside_redundant_parameter_region(
        input,
        &output,
        patch.machine,
        candidate.affected_blocks(),
    ) {
        return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
    }
    let output_region = reconstruct_psi_closed_region_observation(
        &output,
        patch.machine,
        candidate.affected_blocks(),
    )
    .ok_or(OptimizationUnitValidationError::CandidateRegionObservationUnavailable)?;
    if input_region.semantics != output_region.semantics {
        return Err(OptimizationUnitValidationError::CandidateRegionObservationMismatch);
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.redundant-block-parameter.v2",
        ),
        provenance: expected_provenance,
    })
}
