//! The adjacent unique-predecessor block merge: the rule, its eligibility
//! and proposal traversal, and the roster-compaction provenance accounting
//! the merge carries.

use std::collections::BTreeSet;

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    AdjacentBlockMergeRewrite, NodeLocation, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
    PsiRewriteCandidate, ScalarSubstitution,
};
use semantic_vocabulary::BlockId;

use crate::rules::CONTROL_FLOW_CLEANUP_PASS_NAME;
use crate::rules::control_flow_cleanup::block_merge_substitutions::merge_substitutions;
use crate::rules::control_flow_cleanup::merge_boundary_ownership::merge_boundary_ownership_witness;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub struct AdjacentBlockMergeRule;

impl AdjacentBlockMergeRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.adjacent-single-predecessor-block-merge.v5",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            5,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for AdjacentBlockMergeRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
        let Some(AnalysisProduct::Dominators(dominators)) = analyses.get(AnalysisKind::Dominators)
        else {
            return Err(RuleProposalError::MissingAnalysis(AnalysisKind::Dominators));
        };
        let Some(AnalysisProduct::UseDefinition(use_definitions)) =
            analyses.get(AnalysisKind::UseDefinition)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::UseDefinition,
            ));
        };
        let Some(AnalysisProduct::OwnershipFrontiers(frontiers)) =
            analyses.get(AnalysisKind::OwnershipFrontiers)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::OwnershipFrontiers,
            ));
        };
        let mut candidates = Vec::new();
        for function in &unit.functions {
            // Descriptor telescopes require structural substitution, outside this scalar rewrite.
            if function
                .blocks
                .iter()
                .any(|block| !block.structural_parameters.is_empty())
            {
                continue;
            }
            let machine_dominators = dominators
                .functions
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .map(|(_, rows)| rows.as_slice())
                .unwrap_or_default();
            for adjacent in function.blocks.windows(2) {
                let [predecessor, target] = adjacent else {
                    unreachable!("two-block window")
                };
                let eligible_first = target.nodes.first().is_some_and(|node| {
                    (node.successors.is_empty()
                        && (matches!(node.provenance.first(), Some(PsiProvenance::Operation(_)))
                            || (matches!(node.provenance.first(), Some(PsiProvenance::Edge(_)))
                                && matches!(
                                    node.operation,
                                    O::Return { .. }
                                        | O::ReturnUnit { .. }
                                        | O::ReturnStructural { .. }
                                        | O::Crash { .. }
                                ))))
                        || (matches!(node.operation, O::Conditional { .. })
                            && node.successors.len() == 2
                            && node.provenance.is_empty())
                });
                if target.id == function.entry || !eligible_first {
                    continue;
                }
                let Some((predecessor_index, predecessor_node)) = predecessor
                    .nodes
                    .len()
                    .checked_sub(1)
                    .map(|index| (index, &predecessor.nodes[index]))
                else {
                    continue;
                };
                let O::Jump {
                    structural_bindings: _,
                    psi_edge: incoming_edge,
                    target: jump_target,
                    bindings,
                    trivial_affine_discards,
                    residual_affine_discards,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                if !trivial_affine_discards.is_empty()
                    || !residual_affine_discards.is_empty()
                    || *jump_target != target.id
                    || function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.nodes)
                        .flat_map(|node| &node.successors)
                        .filter(|edge| edge.target == target.id)
                        .count()
                        != 1
                {
                    continue;
                }
                let Some(ownership_witness) = merge_boundary_ownership_witness(
                    unit,
                    function,
                    frontiers,
                    *incoming_edge,
                    target.id,
                ) else {
                    continue;
                };
                let Some(substitutions) = merge_substitutions(
                    function.machine,
                    target,
                    bindings,
                    machine_dominators,
                    use_definitions,
                ) else {
                    continue;
                };
                let predecessor_location = NodeLocation {
                    machine: function.machine,
                    block: predecessor.id,
                    node: u32::try_from(predecessor_index)
                        .expect("optimization node index fits u32"),
                };
                let Some((affected_blocks, provenance)) = adjacent_merge_accounting(
                    function,
                    predecessor_location,
                    target.id,
                    &substitutions,
                ) else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_adjacent_block_merge(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
                        provenance,
                        ownership_witness,
                        -2,
                        AdjacentBlockMergeRewrite {
                            predecessor: predecessor_location,
                            incoming_edge: *incoming_edge,
                            target: target.id,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

pub(super) fn adjacent_merge_accounting(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    target: BlockId,
    substitutions: &[ScalarSubstitution],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_position = function
        .blocks
        .iter()
        .position(|block| block.id == predecessor.block)?;
    let target_position = function
        .blocks
        .iter()
        .position(|block| block.id == target)?;
    if target_position != predecessor_position.checked_add(1)? {
        return None;
    }
    let predecessor_node = function.blocks[predecessor_position]
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let incoming = predecessor_node.successors.first()?;
    let target_block = &function.blocks[target_position];
    let incoming_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: incoming.psi_edge,
    };
    let mut affected = BTreeSet::from([predecessor.block, target]);
    let first = target_block.nodes.first()?;
    let mut realized = if !first.provenance.is_empty() {
        vec![ProvenanceRewrite {
            input: incoming_site,
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                NodeLocation {
                    machine: function.machine,
                    block: predecessor.block,
                    node: predecessor.node,
                },
            )),
            sources: incoming.provenance.clone(),
            fuel: incoming.fuel.clone(),
        }]
    } else if !first.successors.is_empty() {
        first
            .successors
            .iter()
            .map(|successor| ProvenanceRewrite {
                input: incoming_site,
                disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: successor.psi_edge,
                }),
                sources: incoming.provenance.clone(),
                fuel: incoming.fuel.clone(),
            })
            .collect()
    } else {
        return None;
    };
    for (node_index, node) in target_block.nodes.iter().enumerate() {
        if node.provenance.is_empty() {
            continue;
        }
        let input = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: target,
            node: u32::try_from(node_index).ok()?,
        });
        let output = PsiRealizationSite::Node(NodeLocation {
            machine: function.machine,
            block: predecessor.block,
            node: predecessor
                .node
                .checked_add(u32::try_from(node_index).ok()?)?,
        });
        realized.push(ProvenanceRewrite {
            input,
            disposition: ProvenanceDisposition::RealizedAt(output),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    for block in function.blocks.iter().skip(target_position + 1) {
        affected.insert(block.id);
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    let substituted_values = substitutions
        .iter()
        .map(|row| row.from)
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        if affected.contains(&block.id) {
            continue;
        }
        let changed_nodes = block
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.uses
                    .iter()
                    .any(|row| substituted_values.contains(&row.value))
            })
            .collect::<Vec<_>>();
        if changed_nodes.is_empty() {
            continue;
        }
        affected.insert(block.id);
        for (node_index, node) in changed_nodes {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            });
            realized.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}
