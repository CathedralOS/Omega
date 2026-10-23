//! Independent constant-conditional replay mechanics.
use crate::candidates::sparse_conditional_constant_propagation::literal_boolean_fact;
use crate::candidates::{
    accept_region_rewrite, candidate_block, candidate_function, canonicalize_provenance,
    set_node_effect_link,
};
use crate::candidates::{provenance_proven_unreachable_at, provenance_realized_at};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    ConstantConditionalRewrite, IntegerEvaluationWitness, NodeLocation, OptimizationEdge,
    ProvenanceDisposition, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch, ValueUse,
    reachable_blocks_after_conditional_fold,
};
use semantic_vocabulary::{BlockId, EdgeId};
use std::collections::BTreeSet;

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
    let function = candidate_function(input, patch.location.machine)?;
    let block = candidate_block(function, patch.location.block)?;
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
    // Removing descriptor roots needs catalog custody beyond scalar pruning.
    if function
        .blocks
        .iter()
        .any(|block| !reachable.contains(&block.id) && !block.structural_parameters.is_empty())
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
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

    accept_region_rewrite(
        input,
        candidate,
        patch.location.machine,
        &expected_blocks,
        b"omega.validator.constant-conditional-fold.v4",
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.location.machine)
                .expect("candidate function exists");
            let output_block = output_function
                .block_mut(patch.location.block)
                .expect("candidate block exists");
            let output_node = &mut output_block.nodes
                [usize::try_from(patch.location.node).expect("u32 fits usize")];
            output_node.operation = O::Jump {
                structural_bindings: selected.structural_bindings.clone(),
                psi_edge: selected.psi_edge,
                target: selected.target,
                bindings: selected.bindings.clone(),
                cleanup_actions: selected.cleanup_actions.clone(),
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
                structural_bindings: selected.structural_bindings.clone(),
                psi_edge: selected.psi_edge,
                target: selected.target,
                bindings: selected.bindings.clone(),
                cleanup_actions: selected.cleanup_actions.clone(),
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
                    effect = set_node_effect_link(node, effect)?;
                }
            }
            Ok(())
        },
    )
}

pub(crate) fn reconstruct_conditional_fold_accounting(
    function: &PsiOptimizationFunction,
    decision: NodeLocation,
    selected_edge: EdgeId,
    rejected_edge: EdgeId,
    reachable: &BTreeSet<BlockId>,
) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
    let decision_node = function
        .block(decision.block)?
        .nodes
        .get(usize::try_from(decision.node).ok()?)?;
    let selected = decision_node.edge(selected_edge)?;
    let rejected = decision_node.edge(rejected_edge)?;
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
    let mut realized = vec![provenance_realized_at(
        selected_site,
        selected_site,
        &selected.provenance,
        &selected.fuel,
    )];
    let mut unreachable = vec![provenance_proven_unreachable_at(
        rejected_site,
        rejected_site,
        &rejected.provenance,
        &rejected.fuel,
    )];
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
                    unreachable.push(provenance_proven_unreachable_at(
                        site,
                        site,
                        &node.provenance,
                        &node.fuel,
                    ));
                }
                for edge in &node.successors {
                    let site = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    unreachable.push(provenance_proven_unreachable_at(
                        site,
                        site,
                        &edge.provenance,
                        &edge.fuel,
                    ));
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
                    realized.push(provenance_realized_at(
                        site,
                        site,
                        &node.provenance,
                        &node.fuel,
                    ));
                }
            }
            expected_effect = expected_effect.checked_add(1)?;
        }
    }
    realized.extend(unreachable);
    canonicalize_provenance(&mut realized);
    Some((affected.into_iter().collect(), realized))
}
