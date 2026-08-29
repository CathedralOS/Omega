//! Redundant-parameter witness reconstruction and rewrite acceptance.

use super::observation::*;
use super::operation_rewrite::rewrite_block_parameter_operation;
use super::*;

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
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.block || patch.parameter == patch.replacement {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
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
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
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
                expected_provenance.push(omega_optimization_unit::ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: edge.provenance.clone(),
                    fuel: edge.fuel.clone(),
                });
            }
            if changes_use {
                affected_blocks.insert(source.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    expected_provenance.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
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
    expected_provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    if incoming != witness.incoming
        || incoming
            .iter()
            .any(|binding| binding.argument != patch.replacement)
    {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    if candidate.substitutions()
        != [omega_optimization_unit::ScalarSubstitution {
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
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .expect("candidate function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.block)
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
    function.facts = reconstruct_fact_index(function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
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
