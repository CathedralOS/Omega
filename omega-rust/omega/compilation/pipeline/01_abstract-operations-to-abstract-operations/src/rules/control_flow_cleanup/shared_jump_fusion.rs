//! Fusion of jumps that share one terminal successor.

use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    NodeLocation, ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiRewriteCandidate, ScalarSubstitution, SharedJumpFusionRewrite,
};
use semantic_vocabulary::{BlockId, EdgeId};

use crate::rules::control_flow_cleanup::merge_boundary_ownership::merge_boundary_ownership_is_identity;
use crate::rules::control_flow_cleanup::scalar_cleanup_functions;
use crate::rules::control_flow_cleanup::{provenance_realized_at, terminal_jump};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    SharedJumpFusionRule,
    crate::rules::control_flow_cleanup::control_flow_cleanup_rule_contract(
        b"omega.psi-rule.shared-terminal-jump-fusion.v2",
        2,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::OwnershipFrontiers,
            AnalysisKind::PostDominators,
        ]),
    ),
);

psi_rule_impl!(SharedJumpFusionRule, |unit, analyses| {
    required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
    required_analysis!(analyses, AnalysisKind::PostDominators => Some(AnalysisProduct::PostDominators(post_dominators)));
    required_analysis!(analyses, AnalysisKind::OwnershipFrontiers => Some(AnalysisProduct::OwnershipFrontiers(frontiers)));
    let mut candidates = Vec::new();
    for function in scalar_cleanup_functions(&unit.functions) {
        let Some((_, function_post_dominators)) = post_dominators.function(function.machine) else {
            continue;
        };
        for predecessor in &function.blocks {
            let Some(jump) = terminal_jump(function, predecessor) else {
                continue;
            };
            let Some(target) = function.block(jump.target) else {
                continue;
            };
            if !function_post_dominators
                .iter()
                .find(|(block, _)| *block == predecessor.id)
                .is_some_and(|(_, blocks)| blocks.contains(&jump.target))
            {
                continue;
            }
            let [terminal] = target.nodes.as_slice() else {
                continue;
            };
            if target.id == function.entry
                || !terminal.successors.is_empty()
                || !matches!(terminal.provenance.first(), Some(PsiProvenance::Edge(_)))
                || !matches!(
                    terminal.operation,
                    abstract_operations::return_or_return_unit_pattern!()
                        | abstract_operations::return_structural_or_crash_pattern!()
                )
            {
                continue;
            }
            let incoming_count = function
                .block_edges()
                .filter(|edge| edge.target == target.id)
                .count();
            if incoming_count < 2
                || !merge_boundary_ownership_is_identity(
                    unit, function, frontiers, jump.edge, target.id,
                )
            {
                continue;
            }
            let Some(mut substitutions) = target
                .parameters
                .iter()
                .zip(jump.bindings)
                .map(|(parameter, binding)| {
                    (binding.parameter == parameter.value
                        && binding.scalar_type == parameter.scalar_type)
                        .then_some(ScalarSubstitution {
                            from: parameter.value,
                            to: binding.argument,
                            scalar_type: parameter.scalar_type,
                        })
                })
                .collect::<Option<Vec<_>>>()
                .filter(|_| target.parameters.len() == jump.bindings.len())
            else {
                continue;
            };
            substitutions.sort();
            let Some((affected_blocks, provenance)) =
                shared_terminal_fusion_accounting(function, jump.location, jump.edge, target.id)
            else {
                continue;
            };
            candidates.push(
                PsiRewriteCandidate::new_shared_jump_fusion(
                    unit.identity,
                    Self::contract(),
                    affected_blocks,
                    substitutions,
                    provenance,
                    -1,
                    SharedJumpFusionRewrite {
                        predecessor: jump.location,
                        incoming_edge: jump.edge,
                        target: target.id,
                    },
                )
                .map_err(RuleProposalError::InvalidCandidate)?,
            );
        }
    }
    Ok(candidates)
});

fn shared_terminal_fusion_accounting(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    incoming_edge: EdgeId,
    target: BlockId,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_block = function.block(predecessor.block)?;
    let incoming = predecessor_block
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?
        .edge(incoming_edge)?;
    let target_block = function.block(target)?;
    let [terminal] = target_block.nodes.as_slice() else {
        return None;
    };
    let input_edge = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: incoming_edge,
    };
    let input_terminal = PsiRealizationSite::Node(NodeLocation {
        machine: function.machine,
        block: target,
        node: 0,
    });
    let output_clone = PsiRealizationSite::Node(predecessor);
    let mut provenance = vec![
        provenance_realized_at(
            input_edge,
            output_clone,
            &incoming.provenance,
            &incoming.fuel,
        ),
        provenance_realized_at(
            input_terminal,
            output_clone,
            &terminal.provenance,
            &terminal.fuel,
        ),
        provenance_realized_at(
            input_terminal,
            input_terminal,
            &terminal.provenance,
            &terminal.fuel,
        ),
    ];
    provenance.sort_by_key(ProvenanceRewrite::canonical_sort_key);
    let mut affected = vec![predecessor.block, target];
    affected.sort();
    affected.dedup();
    Some((affected, provenance))
}
