/*
Control-flow cleanup pass (`control_flow_cleanup`) and its ordered graph
transformation rules: constant-conditional folding, linear and path-qualified
empty-block threading, adjacent and non-adjacent unique-predecessor block
merging, shared-jump fusion, and unreachable private-machine pruning.
*/

use crate::UseDefinitionAnalysis;
use crate::rules::catalog::BuiltInRuleRegistration;
use abstract_operations::{AbstractOperation as O, ValueBinding};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
    OptimizationSafetyClass,
};
use optimization_unit::{
    NodeLocation, OptimizationBlock, OptimizationNode, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiProvenance, PsiRealizationSite, WorkSettlement,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use std::collections::BTreeSet;

pub(crate) mod adjacent_block_merge {
    use std::collections::BTreeSet;

    use abstract_operations::AbstractOperation as O;
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{
        AdjacentBlockMergeRewrite, NodeLocation, ProvenanceRewrite, PsiOptimizationFunction,
        PsiOptimizationUnit, PsiProvenance, PsiRealizationSite, PsiRewriteCandidate,
        ScalarSubstitution,
    };
    use semantic_vocabulary::BlockId;

    use crate::rules::control_flow_cleanup::block_merge_substitutions::merge_substitutions;
    use crate::rules::control_flow_cleanup::merge_boundary_ownership::merge_boundary_ownership_witness;
    use crate::rules::control_flow_cleanup::scalar_cleanup_functions;
    use crate::rules::control_flow_cleanup::{
        merge_candidates, provenance_realized_at, record_block_provenance,
        record_merged_node_positions, record_node_provenance,
    };
    use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        AdjacentBlockMergeRule,
        crate::rules::control_flow_cleanup::control_flow_cleanup_rule_contract(
            b"omega.psi-rule.adjacent-single-predecessor-block-merge.v5",
            5,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
        ),
    );

    psi_rule_impl!(AdjacentBlockMergeRule, |unit, analyses| {
        required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
        required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
        required_analysis!(analyses, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));
        required_analysis!(analyses, AnalysisKind::OwnershipFrontiers => Some(AnalysisProduct::OwnershipFrontiers(frontiers)));
        let mut candidates = Vec::new();
        for function in scalar_cleanup_functions(&unit.functions) {
            let machine_dominators = dominators.rows(function.machine);
            for candidate in merge_candidates(function, |candidate| {
                candidate.target_position == candidate.predecessor_position + 1
                    && candidate.target.nodes.first().is_some_and(|node| {
                        (node.successors.is_empty()
                            && (matches!(
                                node.provenance.first(),
                                Some(PsiProvenance::Operation(_))
                            ) || (matches!(
                                node.provenance.first(),
                                Some(PsiProvenance::Edge(_))
                            ) && matches!(
                                node.operation,
                                abstract_operations::return_or_return_unit_pattern!()
                                    | abstract_operations::return_structural_or_crash_pattern!()
                            ))))
                            || (matches!(node.operation, O::Conditional { .. })
                                && node.successors.len() == 2
                                && node.provenance.is_empty())
                    })
            }) {
                let target = candidate.target;
                let Some(ownership_witness) = merge_boundary_ownership_witness(
                    unit,
                    function,
                    frontiers,
                    candidate.jump.edge,
                    target.id,
                ) else {
                    continue;
                };
                let Some(substitutions) = merge_substitutions(
                    function.machine,
                    target,
                    candidate.jump.bindings,
                    machine_dominators,
                    use_definitions,
                ) else {
                    continue;
                };
                let Some((affected_blocks, provenance)) = adjacent_merge_accounting(
                    function,
                    candidate.jump.location,
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
                            predecessor: candidate.jump.location,
                            incoming_edge: candidate.jump.edge,
                            target: target.id,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    });

    pub(crate) fn adjacent_merge_accounting(
        function: &PsiOptimizationFunction,
        predecessor: NodeLocation,
        target: BlockId,
        substitutions: &[ScalarSubstitution],
    ) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
        let predecessor_position = function.block_index(predecessor.block)?;
        let target_position = function.block_index(target)?;
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
            vec![provenance_realized_at(
                incoming_site,
                PsiRealizationSite::Node(NodeLocation {
                    machine: function.machine,
                    block: predecessor.block,
                    node: predecessor.node,
                }),
                &incoming.provenance,
                &incoming.work,
            )]
        } else if !first.successors.is_empty() {
            first
                .successors
                .iter()
                .map(|successor| {
                    provenance_realized_at(
                        incoming_site,
                        PsiRealizationSite::Edge {
                            machine: function.machine,
                            edge: successor.psi_edge,
                        },
                        &incoming.provenance,
                        &incoming.work,
                    )
                })
                .collect()
        } else {
            return None;
        };
        record_merged_node_positions(function, predecessor, target, target_block, &mut realized)?;
        for block in function.blocks.iter().skip(target_position + 1) {
            affected.insert(block.id);
            record_block_provenance(function.machine, block, &mut realized)?;
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
            record_node_provenance(
                function.machine,
                block,
                changed_nodes.into_iter(),
                &mut realized,
            )?;
        }
        realized.sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
        Some((affected.into_iter().collect(), realized))
    }
}
pub(crate) mod block_merge_substitutions {
    use abstract_operations::ValueBinding;
    use optimization_unit::{OptimizationBlock, ScalarSubstitution};
    use semantic_vocabulary::{BlockId, MachineId};

    use crate::UseDefinitionAnalysis;

    pub(crate) fn merge_substitutions(
        machine: MachineId,
        target: &OptimizationBlock,
        bindings: &[ValueBinding],
        dominators: &[(BlockId, Vec<BlockId>)],
        use_definitions: &UseDefinitionAnalysis,
    ) -> Option<Vec<ScalarSubstitution>> {
        let mut substitutions = target
            .parameters
            .iter()
            .zip(bindings)
            .map(|(parameter, binding)| {
                (binding.parameter == parameter.value
                    && binding.scalar_type == parameter.scalar_type
                    && use_definitions.replacement_dominates_parameter_uses(
                        machine,
                        binding.argument,
                        parameter.value,
                        dominators,
                    ))
                .then_some(ScalarSubstitution {
                    from: parameter.value,
                    to: binding.argument,
                    scalar_type: parameter.scalar_type,
                })
            })
            .collect::<Option<Vec<_>>>()
            .filter(|_| target.parameters.len() == bindings.len())?;
        substitutions.sort();
        Some(substitutions)
    }
}
pub(crate) mod constant_conditional_fold {
    use std::collections::BTreeSet;

    use abstract_operations::AbstractOperation as O;
    use optimization_core::{
        AnalysisKind, AnalysisSet, OptimizationRuleContract, OptimizationSafetyClass,
    };
    use optimization_unit::{
        ConstantConditionalRewrite, NodeLocation, ProvenanceRewrite, PsiOptimizationFunction,
        PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
        reachable_blocks_after_conditional_fold,
    };
    use semantic_vocabulary::{BlockId, EdgeId};

    use crate::rules::control_flow_cleanup::{
        provenance_proven_unreachable_at, provenance_realized_at, record_block_effect_drift,
    };
    use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        ConstantConditionalFoldRule,
        crate::rules::control_flow_cleanup::call_graph_control_flow_cleanup_rule_contract(
            b"omega.psi-rule.constant-conditional-fold.v5",
            5,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::ScalarConstants,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
    );

    psi_rule_impl!(ConstantConditionalFoldRule, |unit, analyses| {
        required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
        required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let O::Conditional {
                        condition,
                        when_true,
                        when_false,
                    } = &node.operation
                    else {
                        continue;
                    };
                    let Some((constant, condition_fact)) =
                        constants.boolean_constant(function.machine, *condition)
                    else {
                        continue;
                    };
                    let (selected, rejected) = if constant {
                        (when_true, when_false)
                    } else {
                        (when_false, when_true)
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).expect("optimization node indices are u32"),
                    };
                    let Some(reachable) = reachable_blocks_after_conditional_fold(
                        function,
                        block.id,
                        selected.psi_edge,
                    ) else {
                        continue;
                    };
                    if function.blocks.iter().any(|block| {
                        !reachable.contains(&block.id) && !block.structural_parameters.is_empty()
                    }) {
                        continue;
                    }
                    let Some((affected_blocks, provenance)) = conditional_fold_accounting(
                        function,
                        location,
                        selected.psi_edge,
                        rejected.psi_edge,
                        &reachable,
                    ) else {
                        continue;
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_constant_conditional(
                            unit.identity,
                            Self::contract(),
                            affected_blocks,
                            provenance,
                            condition_fact,
                            -1,
                            ConstantConditionalRewrite {
                                location,
                                condition: *condition,
                                constant,
                                selected_edge: selected.psi_edge,
                                rejected_edge: rejected.psi_edge,
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    });

    fn conditional_fold_accounting(
        function: &PsiOptimizationFunction,
        decision: NodeLocation,
        selected_edge: EdgeId,
        rejected_edge: EdgeId,
        reachable: &BTreeSet<BlockId>,
    ) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
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
            &selected.work,
        )];
        let mut unreachable = vec![provenance_proven_unreachable_at(
            rejected_site,
            rejected_site,
            &rejected.provenance,
            &rejected.work,
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
                            &node.work,
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
                            &edge.work,
                        ));
                    }
                }
                continue;
            }
            record_block_effect_drift(
                function,
                block,
                &mut expected_effect,
                Some(decision),
                &mut affected,
                &mut realized,
            )?;
        }
        realized.extend(unreachable);
        realized.sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
        Some((affected.into_iter().collect(), realized))
    }
}
pub(crate) mod linear_empty_block_thread {
    use std::collections::BTreeSet;

    use abstract_operations::AbstractOperation as O;
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{
        LinearEmptyBlockRewrite, NodeLocation, ProvenanceRewrite, PsiOptimizationFunction,
        PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    };
    use optimization_unit_semantics::{
        reconstruct_linear_thread_bindings, reconstruct_linear_thread_ownership_is_identity,
    };
    use semantic_vocabulary::BlockId;

    use crate::rules::control_flow_cleanup::scalar_cleanup_functions;
    use crate::rules::control_flow_cleanup::{
        provenance_realized_at, thread_accounting_tail, threadable_empty_blocks,
    };
    use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        LinearEmptyBlockThreadRule,
        crate::rules::control_flow_cleanup::control_flow_cleanup_rule_contract(
            b"omega.psi-rule.linear-empty-block-thread.v2",
            2,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
        ),
    );

    psi_rule_impl!(LinearEmptyBlockThreadRule, |unit, analyses| {
        required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
        required_analysis!(analyses, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));
        required_analysis!(analyses, AnalysisKind::OwnershipFrontiers => Some(_));

        let mut candidates = Vec::new();
        for function in scalar_cleanup_functions(&unit.functions) {
            for empty in threadable_empty_blocks(function, use_definitions) {
                let empty_id = empty.block.id;
                let incoming = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .filter_map(move |(node, candidate)| {
                                candidate
                                    .successors
                                    .iter()
                                    .any(|edge| edge.target == empty_id)
                                    .then_some((block, node, candidate))
                            })
                    })
                    .collect::<Vec<_>>();
                let [(predecessor_block, predecessor_node_index, predecessor_node)] =
                    incoming.as_slice()
                else {
                    continue;
                };
                let O::Jump {
                    structural_bindings: _,
                    psi_edge: incoming_edge,
                    target: predecessor_target,
                    bindings: incoming_bindings,
                    cleanup_actions: incoming_cleanup_actions,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                if !incoming_cleanup_actions.is_empty()
                    || *predecessor_target != empty.block.id
                    || !reconstruct_linear_thread_ownership_is_identity(
                        unit,
                        function,
                        *incoming_edge,
                        empty.block.id,
                        empty.edge,
                        empty.target,
                    )
                    || reconstruct_linear_thread_bindings(
                        &empty.block.parameters,
                        incoming_bindings,
                        empty.bindings,
                    )
                    .is_none()
                {
                    continue;
                }
                let predecessor = NodeLocation {
                    machine: function.machine,
                    block: predecessor_block.id,
                    node: u32::try_from(*predecessor_node_index)
                        .expect("optimization node indices are u32"),
                };
                let Some((affected_blocks, provenance)) =
                    linear_thread_accounting(function, predecessor, empty.location)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_linear_empty_block(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        -3,
                        LinearEmptyBlockRewrite {
                            predecessor,
                            incoming_edge: *incoming_edge,
                            empty: empty.location,
                            outgoing_edge: empty.edge,
                            target: empty.target,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    });

    pub(crate) fn linear_thread_accounting(
        function: &PsiOptimizationFunction,
        predecessor: NodeLocation,
        empty: NodeLocation,
    ) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
        let predecessor_node = function
            .block(predecessor.block)?
            .nodes
            .get(usize::try_from(predecessor.node).ok()?)?;
        let empty_node = function
            .block(empty.block)?
            .nodes
            .get(usize::try_from(empty.node).ok()?)?;
        let predecessor_edge = predecessor_node.successors.first()?;
        let empty_edge = empty_node.successors.first()?;
        let output_site = PsiRealizationSite::Edge {
            machine: function.machine,
            edge: predecessor_edge.psi_edge,
        };
        let empty_site = PsiRealizationSite::Edge {
            machine: function.machine,
            edge: empty_edge.psi_edge,
        };
        let affected = BTreeSet::from([predecessor.block, empty.block]);
        let realized = vec![
            provenance_realized_at(
                output_site,
                output_site,
                &predecessor_edge.provenance,
                &predecessor_edge.work,
            ),
            provenance_realized_at(
                empty_site,
                output_site,
                &empty_edge.provenance,
                &empty_edge.work,
            ),
        ];
        thread_accounting_tail(function, empty.block, Some(predecessor), affected, realized)
    }
}
pub(crate) mod merge_boundary_ownership {
    use optimization_unit::{
        OwnershipFrontierSite, OwnershipFrontierWitness, OwnershipFrontierWitnessRow,
        PsiOptimizationFunction, PsiOptimizationUnit,
    };
    use semantic_vocabulary::{BlockId, EdgeId};

    use crate::OwnershipFrontierAnalysis;

    pub(crate) fn merge_boundary_ownership_is_identity(
        unit: &PsiOptimizationUnit,
        function: &PsiOptimizationFunction,
        frontiers: &OwnershipFrontierAnalysis,
        incoming: EdgeId,
        target: BlockId,
    ) -> bool {
        merge_boundary_ownership_witness(unit, function, frontiers, incoming, target).is_some()
    }

    pub(crate) fn merge_boundary_ownership_witness(
        unit: &PsiOptimizationUnit,
        function: &PsiOptimizationFunction,
        frontiers: &OwnershipFrontierAnalysis,
        incoming: EdgeId,
        target: BlockId,
    ) -> Option<OwnershipFrontierWitness> {
        let sites = [
            OwnershipFrontierSite::EdgeEntry(incoming),
            OwnershipFrontierSite::EdgeExit(incoming),
            OwnershipFrontierSite::BlockEntry(target),
        ];
        let facts = sites.map(|site| frontiers.fact(function.machine, site));
        if facts.iter().all(Option::is_none) {
            return (function.structural_parameters.is_empty()
                && function.entry_claim_declarations.is_empty()
                && function.declared_places.is_empty())
            .then_some(OwnershipFrontierWitness { rows: Vec::new() });
        }
        if !facts.iter().all(|fact| {
            fact.is_some_and(|fact| {
                fact.revision == unit.identity && fact.machine == function.machine
            })
        }) || !facts
            .windows(2)
            .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
        {
            return None;
        }
        let mut rows = facts
            .into_iter()
            .map(|fact| {
                let fact = fact.expect("complete ownership frontier fact set");
                OwnershipFrontierWitnessRow {
                    site: fact.site,
                    fact: fact.identity,
                }
            })
            .collect::<Vec<_>>();
        rows.sort_unstable_by_key(|row| row.site);
        Some(OwnershipFrontierWitness { rows })
    }
}
pub(crate) mod non_adjacent_block_merge {
    use std::collections::{BTreeMap, BTreeSet};

    use abstract_operations::AbstractOperation as O;
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{
        NodeLocation, NonAdjacentBlockMergeRewrite, OptimizationBlock, ProvenanceRewrite,
        PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
        ScalarSubstitution,
    };
    use semantic_vocabulary::BlockId;

    use crate::analyses::block_dominates;
    use crate::rules::control_flow_cleanup::block_merge_substitutions::merge_substitutions;
    use crate::rules::control_flow_cleanup::merge_boundary_ownership::merge_boundary_ownership_is_identity;
    use crate::rules::control_flow_cleanup::scalar_cleanup_functions;
    use crate::rules::control_flow_cleanup::{
        merge_candidates, provenance_realized_at, record_merged_node_positions,
        record_node_provenance,
    };
    use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        NonAdjacentBlockMergeRule,
        crate::rules::control_flow_cleanup::control_flow_cleanup_rule_contract(
            b"omega.psi-rule.non-adjacent-unique-predecessor-block-merge.v1",
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
        ),
    );

    psi_rule_impl!(NonAdjacentBlockMergeRule, |unit, analyses| {
        required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
        required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
        required_analysis!(analyses, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));
        required_analysis!(analyses, AnalysisKind::OwnershipFrontiers => Some(AnalysisProduct::OwnershipFrontiers(frontiers)));

        let mut candidates = Vec::new();
        for function in scalar_cleanup_functions(&unit.functions) {
            let machine_dominators = dominators.rows(function.machine);
            for candidate in merge_candidates(function, |candidate| {
                candidate.target_position != candidate.predecessor_position.saturating_add(1)
                    && non_adjacent_merge_target_is_nonempty(candidate.target)
                    && block_dominates(
                        machine_dominators,
                        candidate.jump.block.id,
                        candidate.target.id,
                    )
            }) {
                let target = candidate.target;
                if !merge_boundary_ownership_is_identity(
                    unit,
                    function,
                    frontiers,
                    candidate.jump.edge,
                    target.id,
                ) {
                    continue;
                }
                let Some(substitutions) = merge_substitutions(
                    function.machine,
                    target,
                    candidate.jump.bindings,
                    machine_dominators,
                    use_definitions,
                ) else {
                    continue;
                };
                let Some((affected_blocks, provenance)) = non_adjacent_merge_accounting(
                    function,
                    candidate.jump.location,
                    target.id,
                    &substitutions,
                ) else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_non_adjacent_block_merge(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        substitutions,
                        provenance,
                        -2,
                        NonAdjacentBlockMergeRewrite {
                            predecessor: candidate.jump.location,
                            incoming_edge: candidate.jump.edge,
                            target: target.id,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    });

    fn non_adjacent_merge_target_is_nonempty(target: &OptimizationBlock) -> bool {
        !target.nodes.is_empty()
            && !matches!(target.nodes.as_slice(), [node] if matches!(node.operation, O::Jump { .. }))
    }

    pub(crate) fn non_adjacent_merge_accounting(
        function: &PsiOptimizationFunction,
        predecessor: NodeLocation,
        target: BlockId,
        substitutions: &[ScalarSubstitution],
    ) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
        let predecessor_position = function.block_index(predecessor.block)?;
        let target_position = function.block_index(target)?;
        if target_position == predecessor_position.checked_add(1)? {
            return None;
        }
        let predecessor_block = &function.blocks[predecessor_position];
        let predecessor_node = predecessor_block
            .nodes
            .get(usize::try_from(predecessor.node).ok()?)?;
        let incoming = predecessor_node.successors.first()?;
        let target_block = &function.blocks[target_position];
        let first = target_block.nodes.first()?;
        let incoming_site = PsiRealizationSite::Edge {
            machine: function.machine,
            edge: incoming.psi_edge,
        };
        let mut realized = if first.successors.is_empty() {
            vec![provenance_realized_at(
                incoming_site,
                PsiRealizationSite::Node(predecessor),
                &incoming.provenance,
                &incoming.work,
            )]
        } else {
            first
                .successors
                .iter()
                .map(|successor| {
                    provenance_realized_at(
                        incoming_site,
                        PsiRealizationSite::Edge {
                            machine: function.machine,
                            edge: successor.psi_edge,
                        },
                        &incoming.provenance,
                        &incoming.work,
                    )
                })
                .collect()
        };

        record_merged_node_positions(function, predecessor, target, target_block, &mut realized)?;

        let mut input_effect = 0u64;
        let mut input_starts = BTreeMap::new();
        for block in &function.blocks {
            input_starts.insert(block.id, input_effect);
            input_effect = input_effect.checked_add(u64::try_from(block.nodes.len()).ok()?)?;
        }
        let mut output_effect = 0u64;
        let mut effect_shifted = BTreeSet::new();
        for block in &function.blocks {
            if block.id == target {
                continue;
            }
            if input_starts.get(&block.id).copied()? != output_effect {
                effect_shifted.insert(block.id);
            }
            let output_nodes = if block.id == predecessor.block {
                block
                    .nodes
                    .len()
                    .checked_sub(1)?
                    .checked_add(target_block.nodes.len())?
            } else {
                block.nodes.len()
            };
            output_effect = output_effect.checked_add(u64::try_from(output_nodes).ok()?)?;
        }

        let substituted_values = substitutions
            .iter()
            .map(|row| row.from)
            .collect::<BTreeSet<_>>();
        let mut affected = BTreeSet::from([predecessor.block, target]);
        affected.extend(effect_shifted.iter().copied());
        for block in &function.blocks {
            if block.id == target {
                continue;
            }
            let mut changed_uses = BTreeSet::new();
            for (node_index, node) in block.nodes.iter().enumerate() {
                if node
                    .uses
                    .iter()
                    .any(|row| substituted_values.contains(&row.value))
                {
                    changed_uses.insert(node_index);
                    affected.insert(block.id);
                }
            }
            let rows = block.nodes.iter().enumerate().filter(|(node_index, node)| {
                !node.provenance.is_empty()
                    && (effect_shifted.contains(&block.id) || changed_uses.contains(node_index))
            });
            record_node_provenance(function.machine, block, rows, &mut realized)?;
        }
        realized.sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
        Some((affected.into_iter().collect(), realized))
    }
}
pub(crate) mod path_qualified_empty_block_thread {
    use std::collections::BTreeSet;

    use abstract_operations::AbstractOperation as O;
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{
        NodeLocation, PathQualifiedEmptyBlockRewrite, ProvenanceRewrite, PsiOptimizationFunction,
        PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    };
    use optimization_unit_semantics::{
        reconstruct_linear_thread_bindings, reconstruct_linear_thread_ownership_is_identity,
    };
    use semantic_vocabulary::{BlockId, EdgeId};

    use crate::rules::control_flow_cleanup::scalar_cleanup_functions;
    use crate::rules::control_flow_cleanup::{
        provenance_realized_at, thread_accounting_tail, threadable_empty_blocks,
    };
    use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        PathQualifiedEmptyBlockThreadRule,
        crate::rules::control_flow_cleanup::control_flow_cleanup_rule_contract(
            b"omega.psi-rule.path-qualified-empty-block-thread.v1",
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::OwnershipFrontiers,
            ]),
        ),
    );

    psi_rule_impl!(PathQualifiedEmptyBlockThreadRule, |unit, analyses| {
        required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(_));
        required_analysis!(analyses, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));
        required_analysis!(analyses, AnalysisKind::OwnershipFrontiers => Some(_));
        let mut candidates = Vec::new();
        for function in scalar_cleanup_functions(&unit.functions) {
            for empty in threadable_empty_blocks(function, use_definitions) {
                let empty_id = empty.block.id;
                let incoming = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .flat_map(move |(node_index, node)| {
                                node.successors
                                    .iter()
                                    .filter(move |edge| edge.target == empty_id)
                                    .map(move |edge| (block, node_index, node, edge))
                            })
                    })
                    .collect::<Vec<_>>();
                if incoming.is_empty()
                    || (incoming.len() == 1 && matches!(incoming[0].2.operation, O::Jump { .. }))
                    || incoming
                        .iter()
                        .any(|(_, _, _, edge)| !edge.cleanup_actions.is_empty())
                {
                    continue;
                }
                if incoming.iter().any(|(_, _, _, edge)| {
                    reconstruct_linear_thread_bindings(
                        &empty.block.parameters,
                        &edge.bindings,
                        empty.bindings,
                    )
                    .is_none()
                        || !reconstruct_linear_thread_ownership_is_identity(
                            unit,
                            function,
                            edge.psi_edge,
                            empty.block.id,
                            empty.edge,
                            empty.target,
                        )
                }) {
                    continue;
                }
                let incoming_edges = incoming
                    .iter()
                    .map(|(_, _, _, edge)| edge.psi_edge)
                    .collect::<Vec<_>>();
                let Some((affected_blocks, provenance)) =
                    path_thread_accounting(function, empty.location, &incoming_edges)
                else {
                    continue;
                };
                candidates.push(
                    PsiRewriteCandidate::new_path_qualified_empty_block(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        -3,
                        PathQualifiedEmptyBlockRewrite {
                            empty: empty.location,
                            outgoing_edge: empty.edge,
                            target: empty.target,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    });

    pub(crate) fn path_thread_accounting(
        function: &PsiOptimizationFunction,
        empty: NodeLocation,
        incoming_edges: &[EdgeId],
    ) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
        let empty_node = function
            .block(empty.block)?
            .nodes
            .get(usize::try_from(empty.node).ok()?)?;
        let outgoing = empty_node.successors.first()?;
        let outgoing_site = PsiRealizationSite::Edge {
            machine: function.machine,
            edge: outgoing.psi_edge,
        };
        let incoming_set = incoming_edges.iter().copied().collect::<BTreeSet<_>>();
        if incoming_set.len() != incoming_edges.len() || incoming_set.is_empty() {
            return None;
        }
        let mut affected = BTreeSet::from([empty.block]);
        let mut realized = Vec::new();
        for block in &function.blocks {
            for node in &block.nodes {
                for edge in &node.successors {
                    if !incoming_set.contains(&edge.psi_edge) || edge.target != empty.block {
                        continue;
                    }
                    affected.insert(block.id);
                    let site = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    realized.push(provenance_realized_at(
                        site,
                        site,
                        &edge.provenance,
                        &edge.work,
                    ));
                    realized.push(provenance_realized_at(
                        outgoing_site,
                        site,
                        &outgoing.provenance,
                        &outgoing.work,
                    ));
                }
            }
        }
        if realized.len() != incoming_edges.len().checked_mul(2)? {
            return None;
        }
        thread_accounting_tail(function, empty.block, None, affected, realized)
    }
}
pub(crate) mod shared_jump_fusion {
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{
        NodeLocation, ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit,
        PsiProvenance, PsiRealizationSite, PsiRewriteCandidate, ScalarSubstitution,
        SharedJumpFusionRewrite,
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
            let Some((_, function_post_dominators)) = post_dominators.function(function.machine)
            else {
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
                let Some((affected_blocks, provenance)) = shared_terminal_fusion_accounting(
                    function,
                    jump.location,
                    jump.edge,
                    target.id,
                ) else {
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
                &incoming.work,
            ),
            provenance_realized_at(
                input_terminal,
                output_clone,
                &terminal.provenance,
                &terminal.work,
            ),
            provenance_realized_at(
                input_terminal,
                input_terminal,
                &terminal.provenance,
                &terminal.work,
            ),
        ];
        provenance.sort_by_key(ProvenanceRewrite::canonical_sort_key);
        let mut affected = vec![predecessor.block, target];
        affected.sort();
        affected.dedup();
        Some((affected, provenance))
    }
}
pub(crate) mod unreachable_private_machine_prune {
    use std::collections::{BTreeMap, BTreeSet};

    use optimization_core::{
        AnalysisKind, AnalysisSet, OptimizationRuleContract, OptimizationSafetyClass,
    };
    use optimization_unit::{
        NodeLocation, ProvenanceRewrite, PrunedMachineCustody, PsiOptimizationUnit,
        PsiRealizationSite, PsiRewriteCandidate, UnreachablePrivateMachinesRewrite,
    };
    use semantic_vocabulary::MachineId;

    use crate::rules::control_flow_cleanup::provenance_proven_unreachable_at;
    use crate::{
        AnalysisProduct, CallGraphAnalysis, PsiOptimizationRule, RuleAnalysisView,
        RuleProposalError,
    };

    psi_rule_contract!(
        UnreachablePrivateMachinePruneRule,
        crate::rules::control_flow_cleanup::call_graph_control_flow_cleanup_rule_contract(
            b"omega.psi-rule.unreachable-private-machine-pruning.v1",
            1,
            AnalysisSet::new([AnalysisKind::CallGraph]),
            OptimizationSafetyClass::StructuralIdentity,
        ),
    );

    psi_rule_impl!(UnreachablePrivateMachinePruneRule, |unit, analyses| {
        required_analysis!(analyses, AnalysisKind::CallGraph => Some(AnalysisProduct::CallGraph(call_graph)));
        let machines = rule_unreachable_private_machine_complement(unit, call_graph);
        if machines.is_empty() {
            return Ok(Vec::new());
        }
        let ordinals = rule_active_source_ordinals(unit);
        let custody = machines
            .iter()
            .map(|machine| PrunedMachineCustody {
                machine: *machine,
                source_ordinal: ordinals[machine],
            })
            .collect::<Vec<_>>();
        let Some(provenance) = rule_pruned_machine_provenance(unit, &machines) else {
            return Ok(Vec::new());
        };
        Ok(vec![
            PsiRewriteCandidate::new_unreachable_private_machines(
                unit.identity,
                Self::contract(),
                Vec::new(),
                provenance,
                -i64::try_from(machines.len()).unwrap_or(i64::MAX),
                UnreachablePrivateMachinesRewrite { machines: custody },
            )
            .map_err(RuleProposalError::InvalidCandidate)?,
        ])
    });

    fn rule_active_source_ordinals(unit: &PsiOptimizationUnit) -> BTreeMap<MachineId, u32> {
        let pruned = unit
            .pruned_machines
            .iter()
            .map(|row| (row.source_ordinal, row.machine))
            .collect::<BTreeMap<_, _>>();
        let mut active = unit.functions.iter();
        let mut result = BTreeMap::new();
        let total = unit.functions.len() + unit.pruned_machines.len();
        for ordinal in 0..total {
            let ordinal = u32::try_from(ordinal).expect("function ordinal fits u32");
            if !pruned.contains_key(&ordinal) {
                let function = active
                    .next()
                    .expect("validated roster has active source member");
                result.insert(function.machine, ordinal);
            }
        }
        result
    }

    pub(crate) fn rule_unreachable_private_machine_complement(
        unit: &PsiOptimizationUnit,
        call_graph: &CallGraphAnalysis,
    ) -> Vec<MachineId> {
        let active = unit
            .functions
            .iter()
            .map(|function| function.machine)
            .collect::<BTreeSet<_>>();
        let mut reachable = BTreeSet::from([unit.entry]);
        reachable.extend(
            unit.dynamic_dispatch
                .tables
                .iter()
                .flat_map(|table| table.rows.iter().copied().flatten()),
        );
        reachable.extend(
            unit.provider_candidates
                .iter()
                .map(|candidate| candidate.candidate),
        );
        reachable.extend(
            unit.functions
                .iter()
                .filter(|function| function.attachment.is_some())
                .map(|function| function.machine),
        );
        let mut references = call_graph
            .callees
            .iter()
            .map(|(machine, callees)| (*machine, callees.iter().copied().collect::<BTreeSet<_>>()))
            .collect::<BTreeMap<_, _>>();
        for function in &unit.functions {
            let selected = references.entry(function.machine).or_default();
            for node in function.block_nodes() {
                selected.extend(unit.operation_machine_references(&node.operation));
            }
        }
        let mut work = reachable.iter().copied().collect::<Vec<_>>();
        while let Some(machine) = work.pop() {
            for callee in references.get(&machine).into_iter().flatten().copied() {
                if active.contains(&callee) && reachable.insert(callee) {
                    work.push(callee);
                }
            }
        }
        active.difference(&reachable).copied().collect()
    }

    fn rule_pruned_machine_provenance(
        unit: &PsiOptimizationUnit,
        machines: &[MachineId],
    ) -> Option<Vec<ProvenanceRewrite>> {
        let machines = machines.iter().copied().collect::<BTreeSet<_>>();
        let mut rows = Vec::new();
        for function in unit
            .functions
            .iter()
            .filter(|function| machines.contains(&function.machine))
        {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let input = PsiRealizationSite::Node(NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).ok()?,
                    });
                    if !node.provenance.is_empty() {
                        rows.push(provenance_proven_unreachable_at(
                            input,
                            input,
                            &node.provenance,
                            &node.work,
                        ));
                    }
                    for edge in &node.successors {
                        let input = PsiRealizationSite::Edge {
                            machine: function.machine,
                            edge: edge.psi_edge,
                        };
                        if !edge.provenance.is_empty() {
                            rows.push(provenance_proven_unreachable_at(
                                input,
                                input,
                                &edge.provenance,
                                &edge.work,
                            ));
                        }
                    }
                }
            }
        }
        rows.sort_by_key(ProvenanceRewrite::canonical_sort_key);
        Some(rows)
    }
}

pub(crate) use adjacent_block_merge::AdjacentBlockMergeRule;
pub(crate) use constant_conditional_fold::ConstantConditionalFoldRule;
pub(crate) use linear_empty_block_thread::LinearEmptyBlockThreadRule;
pub(crate) use non_adjacent_block_merge::NonAdjacentBlockMergeRule;
pub(crate) use path_qualified_empty_block_thread::PathQualifiedEmptyBlockThreadRule;
pub(crate) use shared_jump_fusion::SharedJumpFusionRule;
pub(crate) use unreachable_private_machine_prune::UnreachablePrivateMachinePruneRule;

pub(crate) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, ConstantConditionalFoldRule),
        BuiltInRuleRegistration::new(1, LinearEmptyBlockThreadRule),
        BuiltInRuleRegistration::new(2, PathQualifiedEmptyBlockThreadRule),
        BuiltInRuleRegistration::new(3, AdjacentBlockMergeRule),
        BuiltInRuleRegistration::new(4, SharedJumpFusionRule),
        BuiltInRuleRegistration::new(5, UnreachablePrivateMachinePruneRule),
        BuiltInRuleRegistration::new(6, NonAdjacentBlockMergeRule),
    ]
}

fn control_flow_cleanup_rule_contract(
    identity: &'static [u8],
    version: u32,
    required_analyses: AnalysisSet,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        identity,
        crate::rules::CONTROL_FLOW_CLEANUP_PASS_NAME,
        version,
        required_analyses,
        AnalysisInvalidationSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )
}

fn call_graph_control_flow_cleanup_rule_contract(
    identity: &'static [u8],
    version: u32,
    required_analyses: AnalysisSet,
    safety_class: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        identity,
        crate::rules::CONTROL_FLOW_CLEANUP_PASS_NAME,
        version,
        required_analyses,
        AnalysisInvalidationSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::CallGraph,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        safety_class,
    )
}

pub(crate) fn provenance_realized_at(
    input: PsiRealizationSite,
    realized_at: PsiRealizationSite,
    sources: &[PsiProvenance],
    work: &[WorkSettlement],
) -> ProvenanceRewrite {
    ProvenanceRewrite {
        input,
        disposition: ProvenanceDisposition::RealizedAt(realized_at),
        sources: sources.to_vec(),
        work: work.to_vec(),
    }
}

pub(crate) fn provenance_proven_unreachable_at(
    input: PsiRealizationSite,
    unreachable_at: PsiRealizationSite,
    sources: &[PsiProvenance],
    work: &[WorkSettlement],
) -> ProvenanceRewrite {
    ProvenanceRewrite {
        input,
        disposition: ProvenanceDisposition::ProvenUnreachableAt(unreachable_at),
        sources: sources.to_vec(),
        work: work.to_vec(),
    }
}

pub(crate) fn node_realized_provenance(
    location: NodeLocation,
    node: &OptimizationNode,
) -> ProvenanceRewrite {
    let site = PsiRealizationSite::Node(location);
    provenance_realized_at(site, site, &node.provenance, &node.work)
}

pub(crate) fn record_node_provenance<'node>(
    machine: MachineId,
    block: &OptimizationBlock,
    nodes: impl Iterator<Item = (usize, &'node OptimizationNode)>,
    realized: &mut Vec<ProvenanceRewrite>,
) -> Option<()> {
    for (node_index, node) in nodes {
        if node.provenance.is_empty() {
            continue;
        }
        realized.push(node_realized_provenance(
            NodeLocation {
                machine,
                block: block.id,
                node: u32::try_from(node_index).ok()?,
            },
            node,
        ));
    }
    Some(())
}

pub(crate) fn record_block_provenance(
    machine: MachineId,
    block: &OptimizationBlock,
    realized: &mut Vec<ProvenanceRewrite>,
) -> Option<()> {
    record_node_provenance(machine, block, block.nodes.iter().enumerate(), realized)
}

pub(crate) fn record_merged_node_positions(
    function: &PsiOptimizationFunction,
    predecessor: NodeLocation,
    target: BlockId,
    target_block: &OptimizationBlock,
    realized: &mut Vec<ProvenanceRewrite>,
) -> Option<()> {
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
        realized.push(provenance_realized_at(
            input,
            output,
            &node.provenance,
            &node.work,
        ));
    }
    Some(())
}

pub(crate) fn scalar_cleanup_functions(
    functions: &[PsiOptimizationFunction],
) -> impl Iterator<Item = &PsiOptimizationFunction> {
    functions.iter().filter(|function| {
        function
            .blocks
            .iter()
            .all(|block| block.structural_parameters.is_empty())
    })
}

pub(crate) struct TerminalJump<'a> {
    pub(crate) block: &'a OptimizationBlock,
    pub(crate) location: NodeLocation,
    pub(crate) edge: EdgeId,
    pub(crate) target: BlockId,
    pub(crate) bindings: &'a [ValueBinding],
}

pub(crate) fn terminal_jump<'a>(
    function: &PsiOptimizationFunction,
    block: &'a OptimizationBlock,
) -> Option<TerminalJump<'a>> {
    let (index, node) = block.nodes.iter().enumerate().next_back()?;
    let O::Jump {
        psi_edge,
        target,
        bindings,
        cleanup_actions,
        ..
    } = &node.operation
    else {
        return None;
    };
    if !cleanup_actions.is_empty() {
        return None;
    }
    Some(TerminalJump {
        block,
        location: NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        },
        edge: *psi_edge,
        target: *target,
        bindings,
    })
}

pub(crate) struct MergeCandidate<'a> {
    pub(crate) predecessor_position: usize,
    pub(crate) jump: TerminalJump<'a>,
    pub(crate) target_position: usize,
    pub(crate) target: &'a OptimizationBlock,
}

pub(crate) fn merge_candidates<'a>(
    function: &'a PsiOptimizationFunction,
    admit: impl Fn(&MergeCandidate<'a>) -> bool,
) -> Vec<MergeCandidate<'a>> {
    function
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(predecessor_position, predecessor)| {
            let jump = terminal_jump(function, predecessor)?;
            let (target_position, target) = function
                .blocks
                .iter()
                .enumerate()
                .find(|(_, block)| block.id == jump.target)?;
            let unique_entry = target.id != function.entry
                && function
                    .block_edges()
                    .filter(|edge| edge.target == target.id)
                    .count()
                    == 1;
            let candidate = MergeCandidate {
                predecessor_position,
                jump,
                target_position,
                target,
            };
            (unique_entry && admit(&candidate)).then_some(candidate)
        })
        .collect()
}

pub(crate) fn record_block_effect_drift(
    function: &PsiOptimizationFunction,
    block: &OptimizationBlock,
    expected_effect: &mut u64,
    excluded: Option<NodeLocation>,
    affected: &mut BTreeSet<BlockId>,
    realized: &mut Vec<ProvenanceRewrite>,
) -> Option<()> {
    for (node_index, node) in block.nodes.iter().enumerate() {
        let location = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(node_index).ok()?,
        };
        let effect_changes = node.effect.input != *expected_effect
            || node.effect.output != expected_effect.checked_add(1)?;
        if effect_changes && excluded != Some(location) {
            affected.insert(block.id);
            if !node.provenance.is_empty() {
                realized.push(node_realized_provenance(location, node));
            }
        }
        *expected_effect = expected_effect.checked_add(1)?;
    }
    Some(())
}

pub(crate) fn threadable_empty_blocks<'a>(
    function: &'a PsiOptimizationFunction,
    use_definitions: &'a UseDefinitionAnalysis,
) -> impl Iterator<Item = TerminalJump<'a>> + 'a {
    function.blocks.iter().filter_map(|block| {
        if block.id == function.entry || block.nodes.len() != 1 {
            return None;
        }
        let jump = terminal_jump(function, block)?;
        let parameter_escapes = block.parameters.iter().any(|parameter| {
            use_definitions.uses.iter().any(|(machine, use_site)| {
                *machine == function.machine
                    && use_site.value == parameter.value
                    && (use_site.block != block.id || use_site.node != 0)
            })
        });
        (!parameter_escapes).then_some(jump)
    })
}

pub(crate) fn thread_accounting_tail(
    function: &PsiOptimizationFunction,
    empty: BlockId,
    excluded: Option<NodeLocation>,
    mut affected: BTreeSet<BlockId>,
    mut realized: Vec<ProvenanceRewrite>,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty {
            continue;
        }
        record_block_effect_drift(
            function,
            block,
            &mut expected_effect,
            excluded,
            &mut affected,
            &mut realized,
        )?;
    }
    realized.sort_by_key(ProvenanceRewrite::canonical_sort_key);
    Some((affected.into_iter().collect(), realized))
}
