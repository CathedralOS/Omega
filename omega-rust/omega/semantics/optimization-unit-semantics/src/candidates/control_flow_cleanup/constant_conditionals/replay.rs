//! Independent constant-conditional replay mechanics.

use super::*;

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::FoldConstantConditional(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.location) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(usize::try_from(patch.location.node).expect("u32 fits usize"))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let O::Conditional {
        condition,
        when_true,
        when_false,
    } = &node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let condition_fact = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::unary_operand)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let constant = literal_boolean_fact(function, input.identity, *condition, condition_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let (selected, rejected) = if constant {
        (when_true, when_false)
    } else {
        (when_false, when_true)
    };
    if patch
        != (ConstantConditionalRewrite {
            location: patch.location,
            condition: *condition,
            constant,
            selected_edge: selected.psi_edge,
            rejected_edge: rejected.psi_edge,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let reachable =
        reachable_blocks_after_conditional_fold(function, patch.location.block, selected.psi_edge)
            .ok_or(OptimizationUnitValidationError::CandidateReachabilityMismatch)?;
    let (expected_blocks, accepted_provenance) = reconstruct_conditional_fold_accounting(
        function,
        patch.location,
        selected.psi_edge,
        rejected.psi_edge,
        &reachable,
    )
    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    if candidate.provenance().len() != accepted_provenance.len()
        || candidate
            .provenance()
            .iter()
            .zip(&accepted_provenance)
            .any(|(actual, expected)| {
                actual.input != expected.input
                    || actual.disposition != expected.disposition
                    || actual.sources != expected.sources
            })
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if candidate
        .provenance()
        .iter()
        .zip(&accepted_provenance)
        .any(|(actual, expected)| actual.fuel != expected.fuel)
    {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }
    let selected_site = PsiRealizationSite::Edge {
        machine: patch.location.machine,
        edge: selected.psi_edge,
    };
    let selected_fuel = accepted_provenance
        .iter()
        .find(|row| row.disposition == ProvenanceDisposition::RealizedAt(selected_site))
        .expect("independent accounting includes the selected edge")
        .fuel
        .clone();

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate block exists");
    let output_node =
        &mut output_block.nodes[usize::try_from(patch.location.node).expect("u32 fits usize")];
    output_node.operation = O::Jump {
        psi_edge: selected.psi_edge,
        target: selected.target,
        bindings: selected.bindings.clone(),
        trivial_affine_discards: selected.trivial_affine_discards.clone(),
    };
    output_node.definitions.clear();
    output_node.uses = selected
        .bindings
        .iter()
        .map(|binding| ValueUse {
            value: binding.argument,
            block: patch.location.block,
            node: patch.location.node,
        })
        .collect();
    output_node.successors = vec![OptimizationEdge {
        psi_edge: selected.psi_edge,
        target: selected.target,
        bindings: selected.bindings.clone(),
        trivial_affine_discards: selected.trivial_affine_discards.clone(),
        provenance: vec![PsiProvenance::Edge(selected.psi_edge)],
        fuel: selected_fuel,
    }];
    output_node.ownership.clear();
    output_node.provenance.clear();
    output_node.fuel.clear();
    output_function
        .blocks
        .retain(|block| reachable.contains(&block.id));
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for node in &mut block.nodes {
            node.effect = optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .expect("validated function effect count fits u64"),
            };
            effect = effect
                .checked_add(1)
                .expect("validated function effect count fits u64");
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    refresh_root_service_reach(&mut output)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;

    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .expect("output function exists");
    for input_block in function
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
    {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.constant-conditional-fold.v4",
        ),
        provenance: accepted_provenance,
    })
}

pub(crate) fn reachable_blocks_after_conditional_fold(
    function: &PsiOptimizationFunction,
    source: BlockId,
    selected_edge: EdgeId,
) -> Option<BTreeSet<BlockId>> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block_id) = pending.pop() {
        if !reachable.insert(block_id) {
            continue;
        }
        let block = function.blocks.iter().find(|block| block.id == block_id)?;
        for edge in block.nodes.iter().flat_map(|node| &node.successors) {
            if block_id != source || edge.psi_edge == selected_edge {
                pending.push(edge.target);
            }
        }
    }
    Some(reachable)
}

pub(crate) fn reconstruct_conditional_fold_accounting(
    function: &PsiOptimizationFunction,
    decision: NodeLocation,
    selected_edge: EdgeId,
    rejected_edge: EdgeId,
    reachable: &BTreeSet<BlockId>,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let decision_node = function
        .blocks
        .iter()
        .find(|block| block.id == decision.block)?
        .nodes
        .get(usize::try_from(decision.node).ok()?)?;
    let selected = decision_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == selected_edge)?;
    let rejected = decision_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == rejected_edge)?;
    let selected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: selected_edge,
    };
    let rejected_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: rejected_edge,
    };
    let removed = function
        .blocks
        .iter()
        .map(|block| block.id)
        .filter(|block| !reachable.contains(block))
        .collect::<BTreeSet<_>>();
    let mut affected = BTreeSet::from([decision.block]);
    affected.extend(removed.iter().copied());
    let mut realized = vec![optimization_unit::ProvenanceRewrite {
        input: selected_site,
        disposition: ProvenanceDisposition::RealizedAt(selected_site),
        sources: selected.provenance.clone(),
        fuel: selected.fuel.clone(),
    }];
    let mut unreachable = vec![optimization_unit::ProvenanceRewrite {
        input: rejected_site,
        disposition: ProvenanceDisposition::ProvenUnreachableAt(rejected_site),
        sources: rejected.provenance.clone(),
        fuel: rejected.fuel.clone(),
    }];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if removed.contains(&block.id) {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                };
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    unreachable.push(optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
                for edge in &node.successors {
                    let site = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    unreachable.push(optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(site),
                        sources: edge.provenance.clone(),
                        fuel: edge.fuel.clone(),
                    });
                }
            }
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let location = NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            };
            let effect_changes = node.effect.input != expected_effect
                || node.effect.output != expected_effect.checked_add(1)?;
            if effect_changes && location != decision {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.extend(unreachable);
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}
