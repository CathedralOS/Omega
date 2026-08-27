use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, Optimization, OptimizationExecutionPhase,
    OptimizationPassIdentity, OptimizationRuleContract, OptimizationRuleIdentity,
    OptimizationSafetyClass, OptimizationSelections, ScalarConstantFactIdentity,
};
use omega_optimization_unit::{
    BlockParameterIncomingBinding, BooleanConstantRewrite, ConstantConditionalRewrite,
    IntegerConstantRewrite, IntegerEvaluationWitness, LinearEmptyBlockRewrite, NodeLocation,
    OptimizationFact, OwnershipFrontierSite, PathQualifiedEmptyBlockRewrite, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    RedundantBlockParameterRewrite, RedundantBlockParameterWitness,
};
use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
use psi_core::{BlockId, IntegerValue, MachineId, OperationId, ValueId};

use crate::{
    AnalysisProduct, OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError, ScalarConstant, ScalarConstantAnalysis,
};

const SCCP_PASS_NAME: &[u8] = b"omega.psi-pass.sparse-conditional-constant-propagation.v1";
const CONTROL_FLOW_CLEANUP_PASS_NAME: &[u8] = b"omega.psi-pass.control-flow-cleanup.v5";
const COPY_PROPAGATION_PASS_NAME: &[u8] = b"omega.psi-pass.copy-propagation.v1";

#[derive(Debug, Clone, Copy, Default)]
pub struct ConstantConditionalFoldRule;

impl ConstantConditionalFoldRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.constant-conditional-fold.v5",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            5,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::ScalarConstants,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::CallGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LinearEmptyBlockThreadRule;

impl LinearEmptyBlockThreadRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.linear-empty-block-thread.v2",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            2,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
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

impl PsiOptimizationRule for LinearEmptyBlockThreadRule {
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
            for empty in &function.blocks {
                if empty.id == function.entry || empty.nodes.len() != 1 {
                    continue;
                }
                let O::Jump {
                    psi_edge: outgoing_edge,
                    target,
                    bindings: outgoing_bindings,
                } = &empty.nodes[0].operation
                else {
                    continue;
                };
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
                                    .any(|edge| edge.target == empty.id)
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
                    psi_edge: incoming_edge,
                    target: predecessor_target,
                    bindings: incoming_bindings,
                } = &predecessor_node.operation
                else {
                    continue;
                };
                if *predecessor_target != empty.id
                    || empty.parameters.iter().any(|parameter| {
                        use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine
                                && use_site.value == parameter.value
                                && (use_site.block != empty.id || use_site.node != 0)
                        })
                    })
                    || !linear_thread_ownership_is_identity(
                        unit,
                        function,
                        frontiers,
                        *incoming_edge,
                        empty.id,
                        *outgoing_edge,
                        *target,
                    )
                {
                    continue;
                }
                let Some(_) = compose_linear_thread_bindings(
                    &empty.parameters,
                    incoming_bindings,
                    outgoing_bindings,
                ) else {
                    continue;
                };
                let predecessor = NodeLocation {
                    machine: function.machine,
                    block: predecessor_block.id,
                    node: u32::try_from(*predecessor_node_index)
                        .expect("optimization node indices are u32"),
                };
                let empty_location = NodeLocation {
                    machine: function.machine,
                    block: empty.id,
                    node: 0,
                };
                let Some((affected_blocks, provenance)) =
                    linear_thread_accounting(function, predecessor, empty_location)
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
                            empty: empty_location,
                            outgoing_edge: *outgoing_edge,
                            target: *target,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PathQualifiedEmptyBlockThreadRule;

impl PathQualifiedEmptyBlockThreadRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.path-qualified-empty-block-thread.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(CONTROL_FLOW_CLEANUP_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
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

impl PsiOptimizationRule for PathQualifiedEmptyBlockThreadRule {
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
            for empty in &function.blocks {
                if empty.id == function.entry || empty.nodes.len() != 1 {
                    continue;
                }
                let O::Jump {
                    psi_edge: outgoing_edge,
                    target,
                    bindings: outgoing_bindings,
                } = &empty.nodes[0].operation
                else {
                    continue;
                };
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
                                    .filter(move |edge| edge.target == empty.id)
                                    .map(move |edge| (block, node_index, node, edge))
                            })
                    })
                    .collect::<Vec<_>>();
                if incoming.is_empty()
                    || (incoming.len() == 1 && matches!(incoming[0].2.operation, O::Jump { .. }))
                    || empty.parameters.iter().any(|parameter| {
                        use_definitions.uses.iter().any(|(machine, use_site)| {
                            *machine == function.machine
                                && use_site.value == parameter.value
                                && (use_site.block != empty.id || use_site.node != 0)
                        })
                    })
                {
                    continue;
                }
                if incoming.iter().any(|(_, _, _, edge)| {
                    compose_linear_thread_bindings(
                        &empty.parameters,
                        &edge.bindings,
                        outgoing_bindings,
                    )
                    .is_none()
                        || !linear_thread_ownership_is_identity(
                            unit,
                            function,
                            frontiers,
                            edge.psi_edge,
                            empty.id,
                            *outgoing_edge,
                            *target,
                        )
                }) {
                    continue;
                }
                let empty_location = NodeLocation {
                    machine: function.machine,
                    block: empty.id,
                    node: 0,
                };
                let incoming_edges = incoming
                    .iter()
                    .map(|(_, _, _, edge)| edge.psi_edge)
                    .collect::<Vec<_>>();
                let Some((affected_blocks, provenance)) =
                    path_thread_accounting(function, empty_location, &incoming_edges)
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
                            empty: empty_location,
                            outgoing_edge: *outgoing_edge,
                            target: *target,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

impl PsiOptimizationRule for ConstantConditionalFoldRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let Some(AnalysisProduct::ScalarConstants(constants)) =
            analyses.get(AnalysisKind::ScalarConstants)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ScalarConstants,
            ));
        };
        if analyses.get(AnalysisKind::ControlFlowGraph).is_none() {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ControlFlowGraph,
            ));
        }
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
                        boolean_constant(constants, function.machine, *condition)
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
                    let Some(reachable) =
                        reachable_blocks_after_fold(function, block.id, selected.psi_edge)
                    else {
                        continue;
                    };
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
    }
}

fn reachable_blocks_after_fold(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    source: BlockId,
    selected_edge: psi_core::EdgeId,
) -> Option<BTreeSet<BlockId>> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block_id) = pending.pop() {
        if !reachable.insert(block_id) {
            continue;
        }
        let Some(block) = function.blocks.iter().find(|block| block.id == block_id) else {
            return None;
        };
        for edge in block.nodes.iter().flat_map(|node| &node.successors) {
            if block_id != source || edge.psi_edge == selected_edge {
                pending.push(edge.target);
            }
        }
    }
    Some(reachable)
}

fn conditional_fold_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    decision: NodeLocation,
    selected_edge: psi_core::EdgeId,
    rejected_edge: psi_core::EdgeId,
    reachable: &BTreeSet<BlockId>,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
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
    let mut realized = vec![ProvenanceRewrite {
        input: selected_site,
        disposition: ProvenanceDisposition::RealizedAt(selected_site),
        sources: selected.provenance.clone(),
        fuel: selected.fuel.clone(),
    }];
    let mut unreachable = vec![ProvenanceRewrite {
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
                    unreachable.push(ProvenanceRewrite {
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
                    unreachable.push(ProvenanceRewrite {
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
                    realized.push(ProvenanceRewrite {
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

fn compose_linear_thread_bindings(
    parameters: &[omega_optimization_unit::ValueDefinition],
    incoming: &[omega_terminal_abstract_operations::TerminalValueBinding],
    outgoing: &[omega_terminal_abstract_operations::TerminalValueBinding],
) -> Option<Vec<omega_terminal_abstract_operations::TerminalValueBinding>> {
    if parameters.len() != incoming.len() {
        return None;
    }
    let replacements = parameters
        .iter()
        .zip(incoming)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some((parameter.value, (binding.argument, binding.scalar_type)))
        })
        .collect::<Option<BTreeMap<_, _>>>()?;
    Some(
        outgoing
            .iter()
            .map(|binding| {
                replacements
                    .get(&binding.argument)
                    .map_or(*binding, |(argument, scalar_type)| {
                        omega_terminal_abstract_operations::TerminalValueBinding {
                            parameter: binding.parameter,
                            argument: *argument,
                            scalar_type: *scalar_type,
                        }
                    })
            })
            .collect(),
    )
}

fn linear_thread_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &omega_optimization_unit::PsiOptimizationFunction,
    frontiers: &crate::OwnershipFrontierAnalysis,
    incoming: psi_core::EdgeId,
    empty: BlockId,
    outgoing: psi_core::EdgeId,
    target: BlockId,
) -> bool {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(empty),
        OwnershipFrontierSite::EdgeEntry(outgoing),
        OwnershipFrontierSite::EdgeExit(outgoing),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| frontiers.fact(function.machine, site));
    if facts.iter().all(Option::is_none) {
        return function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty();
    }
    facts.iter().all(|fact| {
        fact.is_some_and(|fact| fact.revision == unit.identity && fact.machine == function.machine)
    }) && facts
        .windows(2)
        .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
}

fn linear_thread_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    predecessor: NodeLocation,
    empty: NodeLocation,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let predecessor_node = function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)?
        .nodes
        .get(usize::try_from(predecessor.node).ok()?)?;
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
        .nodes
        .get(usize::try_from(empty.node).ok()?)?;
    let predecessor_edge = predecessor_node.successors.first()?;
    let empty_edge = empty_node.successors.first()?;
    let output_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: predecessor_edge.psi_edge,
    };
    let predecessor_site = output_site;
    let empty_site = PsiRealizationSite::Edge {
        machine: function.machine,
        edge: empty_edge.psi_edge,
    };

    let mut affected = BTreeSet::from([predecessor.block, empty.block]);
    let mut realized = vec![
        ProvenanceRewrite {
            input: predecessor_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: predecessor_edge.provenance.clone(),
            fuel: predecessor_edge.fuel.clone(),
        },
        ProvenanceRewrite {
            input: empty_site,
            disposition: ProvenanceDisposition::RealizedAt(output_site),
            sources: empty_edge.provenance.clone(),
            fuel: empty_edge.fuel.clone(),
        },
    ];
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
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
            if effect_changes && location != predecessor {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(ProvenanceRewrite {
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
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

fn path_thread_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    empty: NodeLocation,
    incoming_edges: &[psi_core::EdgeId],
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let empty_node = function
        .blocks
        .iter()
        .find(|block| block.id == empty.block)?
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
                realized.push(ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: edge.provenance.clone(),
                    fuel: edge.fuel.clone(),
                });
                realized.push(ProvenanceRewrite {
                    input: outgoing_site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: outgoing.provenance.clone(),
                    fuel: outgoing.fuel.clone(),
                });
            }
        }
    }
    if realized.len() != incoming_edges.len().checked_mul(2)? {
        return None;
    }
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        if block.id == empty.block {
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
            if effect_changes {
                affected.insert(block.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    realized.push(ProvenanceRewrite {
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
    realized.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected.into_iter().collect(), realized))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RedundantBlockParameterRule;

impl RedundantBlockParameterRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.redundant-block-parameter.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(COPY_PROPAGATION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for RedundantBlockParameterRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_redundant_block_parameters(unit, analyses, Self::contract())
    }
}

fn integer_evaluation_contract(
    rule_name: &[u8],
    safety_class: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(rule_name),
        OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME),
        1,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        safety_class,
    )
    .expect("built-in rule has nonzero version")
}

macro_rules! integer_evaluation_rule {
    ($name:ident, $rule_name:literal, $kind:expr, $safety:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                integer_evaluation_contract($rule_name, $safety)
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                propose_integer_binary_constants(unit, analyses, Self::contract(), $kind)
            }
        }
    };
}

integer_evaluation_rule!(
    ExactIntegerAddConstantsRule,
    b"omega.psi-rule.exact-integer-add-constants.v1",
    IntegerBinaryKind::ExactAdd,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerSubtractConstantsRule,
    b"omega.psi-rule.exact-integer-subtract-constants.v1",
    IntegerBinaryKind::ExactSubtract,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerMultiplyConstantsRule,
    b"omega.psi-rule.exact-integer-multiply-constants.v1",
    IntegerBinaryKind::ExactMultiply,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    WrappingIntegerAddConstantsRule,
    b"omega.psi-rule.wrapping-integer-add-constants.v1",
    IntegerBinaryKind::WrappingAdd,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    WrappingIntegerSubtractConstantsRule,
    b"omega.psi-rule.wrapping-integer-subtract-constants.v1",
    IntegerBinaryKind::WrappingSubtract,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    WrappingIntegerMultiplyConstantsRule,
    b"omega.psi-rule.wrapping-integer-multiply-constants.v1",
    IntegerBinaryKind::WrappingMultiply,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    SaturatingIntegerAddConstantsRule,
    b"omega.psi-rule.saturating-integer-add-constants.v1",
    IntegerBinaryKind::SaturatingAdd,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    SaturatingIntegerSubtractConstantsRule,
    b"omega.psi-rule.saturating-integer-subtract-constants.v1",
    IntegerBinaryKind::SaturatingSubtract,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    SaturatingIntegerMultiplyConstantsRule,
    b"omega.psi-rule.saturating-integer-multiply-constants.v1",
    IntegerBinaryKind::SaturatingMultiply,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    ExactIntegerDivideConstantsRule,
    b"omega.psi-rule.exact-integer-divide-constants.v1",
    IntegerBinaryKind::ExactDivide,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerRemainderConstantsRule,
    b"omega.psi-rule.exact-integer-remainder-constants.v1",
    IntegerBinaryKind::ExactRemainder,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    WrappingIntegerDivideConstantsRule,
    b"omega.psi-rule.wrapping-integer-divide-constants.v1",
    IntegerBinaryKind::WrappingDivide,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    WrappingIntegerRemainderConstantsRule,
    b"omega.psi-rule.wrapping-integer-remainder-constants.v1",
    IntegerBinaryKind::WrappingRemainder,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    SaturatingIntegerDivideConstantsRule,
    b"omega.psi-rule.saturating-integer-divide-constants.v1",
    IntegerBinaryKind::SaturatingDivide,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    SaturatingIntegerRemainderConstantsRule,
    b"omega.psi-rule.saturating-integer-remainder-constants.v1",
    IntegerBinaryKind::SaturatingRemainder,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerShiftLeftConstantsRule,
    b"omega.psi-rule.exact-integer-shift-left-constants.v1",
    IntegerBinaryKind::ExactShiftLeft,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    ExactIntegerShiftRightConstantsRule,
    b"omega.psi-rule.exact-integer-shift-right-constants.v1",
    IntegerBinaryKind::ExactShiftRight,
    OptimizationSafetyClass::ProofCertified
);
integer_evaluation_rule!(
    WrappingIntegerShiftLeftConstantsRule,
    b"omega.psi-rule.wrapping-integer-shift-left-constants.v1",
    IntegerBinaryKind::WrappingShiftLeft,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    WrappingIntegerShiftRightConstantsRule,
    b"omega.psi-rule.wrapping-integer-shift-right-constants.v1",
    IntegerBinaryKind::WrappingShiftRight,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    IntegerBitwiseAndConstantsRule,
    b"omega.psi-rule.integer-bitwise-and-constants.v1",
    IntegerBinaryKind::BitwiseAnd,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    IntegerBitwiseOrConstantsRule,
    b"omega.psi-rule.integer-bitwise-or-constants.v1",
    IntegerBinaryKind::BitwiseOr,
    OptimizationSafetyClass::ExactOperationSemantics
);
integer_evaluation_rule!(
    IntegerBitwiseXorConstantsRule,
    b"omega.psi-rule.integer-bitwise-xor-constants.v1",
    IntegerBinaryKind::BitwiseXor,
    OptimizationSafetyClass::ExactOperationSemantics
);

macro_rules! boolean_evaluation_rule {
    ($name:ident, $rule_name:literal, $kind:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                integer_evaluation_contract(
                    $rule_name,
                    OptimizationSafetyClass::ExactOperationSemantics,
                )
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                propose_boolean_constants(unit, analyses, Self::contract(), $kind)
            }
        }
    };
}

boolean_evaluation_rule!(
    BooleanNotConstantsRule,
    b"omega.psi-rule.boolean-not-constants.v1",
    BooleanEvaluationKind::Not
);
boolean_evaluation_rule!(
    BooleanEqualConstantsRule,
    b"omega.psi-rule.boolean-equal-constants.v1",
    BooleanEvaluationKind::Equal
);
boolean_evaluation_rule!(
    IntegerEqualConstantsRule,
    b"omega.psi-rule.integer-equal-constants.v1",
    BooleanEvaluationKind::IntegerEqual
);
boolean_evaluation_rule!(
    IntegerLessThanConstantsRule,
    b"omega.psi-rule.integer-less-than-constants.v1",
    BooleanEvaluationKind::IntegerLessThan
);
boolean_evaluation_rule!(
    IntegerLessOrEqualConstantsRule,
    b"omega.psi-rule.integer-less-or-equal-constants.v1",
    BooleanEvaluationKind::IntegerLessOrEqual
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BooleanEvaluationKind {
    Not,
    Equal,
    IntegerEqual,
    IntegerLessThan,
    IntegerLessOrEqual,
}

fn propose_boolean_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: BooleanEvaluationKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let (source_operation, result, constant, witness) = match (&node.operation, kind) {
                    (
                        O::BooleanNot {
                            psi_operation,
                            result,
                            operand,
                        },
                        BooleanEvaluationKind::Not,
                    ) => {
                        let Some((operand, operand_fact)) =
                            boolean_constant(constants, function.machine, *operand)
                        else {
                            continue;
                        };
                        (
                            *psi_operation,
                            *result,
                            !operand,
                            IntegerEvaluationWitness::Unary { operand_fact },
                        )
                    }
                    (
                        O::BooleanEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                        BooleanEvaluationKind::Equal,
                    ) => {
                        let Some((left, left_fact)) =
                            boolean_constant(constants, function.machine, *left)
                        else {
                            continue;
                        };
                        let Some((right, right_fact)) =
                            boolean_constant(constants, function.machine, *right)
                        else {
                            continue;
                        };
                        (
                            *psi_operation,
                            *result,
                            left == right,
                            IntegerEvaluationWitness::Binary {
                                left_fact,
                                right_fact,
                            },
                        )
                    }
                    (
                        O::IntegerEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                        BooleanEvaluationKind::IntegerEqual,
                    )
                    | (
                        O::IntegerLessThan {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                        BooleanEvaluationKind::IntegerLessThan,
                    )
                    | (
                        O::IntegerLessOrEqual {
                            psi_operation,
                            result,
                            left,
                            right,
                        },
                        BooleanEvaluationKind::IntegerLessOrEqual,
                    ) => {
                        let Some((left_value, left_fact)) =
                            integer_constant(constants, function.machine, *left)
                        else {
                            continue;
                        };
                        let Some((right_value, right_fact)) =
                            integer_constant(constants, function.machine, *right)
                        else {
                            continue;
                        };
                        let Some(left_type) = integer_value_type(function, *left) else {
                            continue;
                        };
                        if integer_value_type(function, *right) != Some(left_type) {
                            continue;
                        }
                        let Some(ordering) = left_type.compare(left_value, right_value) else {
                            continue;
                        };
                        let constant = match kind {
                            BooleanEvaluationKind::IntegerEqual => ordering.is_eq(),
                            BooleanEvaluationKind::IntegerLessThan => ordering.is_lt(),
                            BooleanEvaluationKind::IntegerLessOrEqual => !ordering.is_gt(),
                            _ => unreachable!(),
                        };
                        (
                            *psi_operation,
                            *result,
                            constant,
                            IntegerEvaluationWitness::Binary {
                                left_fact,
                                right_fact,
                            },
                        )
                    }
                    _ => continue,
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_boolean_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        witness,
                        -1,
                        BooleanConstantRewrite {
                            location,
                            source_operation,
                            result,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExactIntegerCastConstantsRule;

impl ExactIntegerCastConstantsRule {
    pub fn contract() -> OptimizationRuleContract {
        integer_evaluation_contract(
            b"omega.psi-rule.exact-integer-cast-constants.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for ExactIntegerCastConstantsRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_exact_integer_cast_constants(unit, analyses, Self::contract())
    }
}

macro_rules! integer_unary_rule {
    ($name:ident, $rule_name:literal, $kind:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                integer_evaluation_contract(
                    $rule_name,
                    OptimizationSafetyClass::ExactOperationSemantics,
                )
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                propose_integer_unary_constants(unit, analyses, Self::contract(), $kind)
            }
        }
    };
}

integer_unary_rule!(
    IntegerWidenConstantsRule,
    b"omega.psi-rule.integer-widen-constants.v1",
    IntegerUnaryKind::Widen
);
integer_unary_rule!(
    IntegerBitwiseNotConstantsRule,
    b"omega.psi-rule.integer-bitwise-not-constants.v1",
    IntegerUnaryKind::BitwiseNot
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegerUnaryKind {
    Widen,
    BitwiseNot,
}

fn propose_integer_unary_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: IntegerUnaryKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let (source_operation, result, source_type, target_type, operand) =
                    match (&node.operation, kind) {
                        (
                            O::IntegerWiden {
                                psi_operation,
                                result,
                                source_type,
                                target_type,
                                operand,
                            },
                            IntegerUnaryKind::Widen,
                        ) => (
                            *psi_operation,
                            *result,
                            *source_type,
                            *target_type,
                            *operand,
                        ),
                        (
                            O::IntegerBitwiseNot {
                                psi_operation,
                                result,
                                scalar_type,
                                operand,
                            },
                            IntegerUnaryKind::BitwiseNot,
                        ) => (
                            *psi_operation,
                            *result,
                            *scalar_type,
                            *scalar_type,
                            *operand,
                        ),
                        _ => continue,
                    };
                let Some((operand_value, operand_fact)) =
                    integer_constant(constants, function.machine, operand)
                else {
                    #[cfg(test)]
                    eprintln!("linear thread refused accounting");
                    continue;
                };
                let constant = match kind {
                    IntegerUnaryKind::Widen => {
                        source_type.widen_value_to(target_type, operand_value)
                    }
                    IntegerUnaryKind::BitwiseNot => source_type.bitwise_not(operand_value),
                };
                let Some(constant) = constant else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        IntegerEvaluationWitness::Unary { operand_fact },
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation,
                            result,
                            scalar_type: target_type,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn propose_exact_integer_cast_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let O::IntegerExactCast {
                    psi_operation,
                    result,
                    source_type,
                    target_type,
                    operand,
                    ..
                } = node.operation
                else {
                    continue;
                };
                let Some((operand_value, operand_fact)) =
                    integer_constant(constants, function.machine, operand)
                else {
                    continue;
                };
                let Some(constant) = source_type.exact_cast_value_to(target_type, operand_value)
                else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        proof_certified_unary_witness(
                            unit,
                            function.machine,
                            psi_operation,
                            operand_fact,
                        )?,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: psi_operation,
                            result,
                            scalar_type: target_type,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn propose_integer_binary_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: IntegerBinaryKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(shape) = integer_binary_shape(&node.operation) else {
                    continue;
                };
                if shape.kind != kind {
                    continue;
                }
                let Some((left_value, left_fact)) =
                    integer_constant(constants, function.machine, shape.left)
                else {
                    continue;
                };
                let Some((right_value, right_fact)) =
                    integer_constant(constants, function.machine, shape.right)
                else {
                    continue;
                };
                let Some(constant) = shape.evaluate(left_value, right_value) else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        integer_binary_witness(
                            unit,
                            function.machine,
                            shape.source,
                            contract.safety_class(),
                            left_fact,
                            right_fact,
                        )?,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: shape.source,
                            result: shape.result,
                            scalar_type: shape.scalar_type,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn accepted_obligation_fact(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
) -> Result<omega_optimization_core::AcceptedObligationFactIdentity, RuleProposalError> {
    let obligation = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .and_then(|function| {
            function.facts.iter().find_map(|fact| match fact {
                OptimizationFact::OperationObligationReference {
                    obligation,
                    support,
                } if *support == operation => Some(*obligation),
                _ => None,
            })
        });
    let Some(obligation) = obligation else {
        return Err(RuleProposalError::MissingAcceptedObligation {
            machine,
            operation,
            obligation: None,
        });
    };
    unit.accepted_obligation_facts
        .iter()
        .find(|fact| {
            fact.machine == machine && fact.operation == operation && fact.obligation == obligation
        })
        .map(|fact| fact.identity)
        .ok_or(RuleProposalError::MissingAcceptedObligation {
            machine,
            operation,
            obligation: Some(obligation),
        })
}

fn proof_certified_unary_witness(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
    operand_fact: ScalarConstantFactIdentity,
) -> Result<IntegerEvaluationWitness, RuleProposalError> {
    Ok(IntegerEvaluationWitness::ProofCertifiedUnary {
        operand_fact,
        obligation_fact: accepted_obligation_fact(unit, machine, operation)?,
    })
}

fn integer_binary_witness(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
    safety: OptimizationSafetyClass,
    left_fact: ScalarConstantFactIdentity,
    right_fact: ScalarConstantFactIdentity,
) -> Result<IntegerEvaluationWitness, RuleProposalError> {
    if safety == OptimizationSafetyClass::ProofCertified {
        Ok(IntegerEvaluationWitness::ProofCertifiedBinary {
            left_fact,
            right_fact,
            obligation_fact: accepted_obligation_fact(unit, machine, operation)?,
        })
    } else {
        Ok(IntegerEvaluationWitness::Binary {
            left_fact,
            right_fact,
        })
    }
}

struct IntegerBinaryShape {
    source: OperationId,
    result: ValueId,
    scalar_type: psi_core::IntegerType,
    left: ValueId,
    right: ValueId,
    count_type: Option<psi_core::IntegerType>,
    kind: IntegerBinaryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegerBinaryKind {
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    WrappingAdd,
    WrappingSubtract,
    WrappingMultiply,
    SaturatingAdd,
    SaturatingSubtract,
    SaturatingMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    ExactShiftLeft,
    ExactShiftRight,
    WrappingShiftLeft,
    WrappingShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

impl IntegerBinaryShape {
    fn evaluate(&self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        match self.kind {
            IntegerBinaryKind::ExactAdd => self.scalar_type.exact_add(left, right),
            IntegerBinaryKind::ExactSubtract => self.scalar_type.exact_sub(left, right),
            IntegerBinaryKind::ExactMultiply => self.scalar_type.exact_mul(left, right),
            IntegerBinaryKind::WrappingAdd => self.scalar_type.wrapping_add(left, right),
            IntegerBinaryKind::WrappingSubtract => self.scalar_type.wrapping_sub(left, right),
            IntegerBinaryKind::WrappingMultiply => self.scalar_type.wrapping_mul(left, right),
            IntegerBinaryKind::SaturatingAdd => self.scalar_type.saturating_add(left, right),
            IntegerBinaryKind::SaturatingSubtract => self.scalar_type.saturating_sub(left, right),
            IntegerBinaryKind::SaturatingMultiply => self.scalar_type.saturating_mul(left, right),
            IntegerBinaryKind::ExactDivide => self.scalar_type.exact_div(left, right),
            IntegerBinaryKind::ExactRemainder => self.scalar_type.exact_rem(left, right),
            IntegerBinaryKind::WrappingDivide => self.scalar_type.wrapping_div(left, right),
            IntegerBinaryKind::WrappingRemainder => self.scalar_type.wrapping_rem(left, right),
            IntegerBinaryKind::SaturatingDivide => self.scalar_type.saturating_div(left, right),
            IntegerBinaryKind::SaturatingRemainder => self.scalar_type.saturating_rem(left, right),
            IntegerBinaryKind::ExactShiftLeft => self.scalar_type.exact_shift_left(
                left,
                self.count_type.expect("shift count type"),
                right,
            ),
            IntegerBinaryKind::ExactShiftRight => self.scalar_type.exact_shift_right(
                left,
                self.count_type.expect("shift count type"),
                right,
            ),
            IntegerBinaryKind::WrappingShiftLeft => self.scalar_type.wrapping_shift_left(
                left,
                self.count_type.expect("shift count type"),
                right,
            ),
            IntegerBinaryKind::WrappingShiftRight => self.scalar_type.wrapping_shift_right(
                left,
                self.count_type.expect("shift count type"),
                right,
            ),
            IntegerBinaryKind::BitwiseAnd => self.scalar_type.bitwise_and(left, right),
            IntegerBinaryKind::BitwiseOr => self.scalar_type.bitwise_or(left, right),
            IntegerBinaryKind::BitwiseXor => self.scalar_type.bitwise_xor(left, right),
        }
    }
}

fn integer_binary_shape(operation: &O) -> Option<IntegerBinaryShape> {
    let shift = match operation {
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => Some((
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::ExactShiftLeft,
        )),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => Some((
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::ExactShiftRight,
        )),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => Some((
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::WrappingShiftLeft,
        )),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => Some((
            *psi_operation,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
            IntegerBinaryKind::WrappingShiftRight,
        )),
        _ => None,
    };
    if let Some((source, result, scalar_type, count_type, left, right, kind)) = shift {
        return Some(IntegerBinaryShape {
            source,
            result,
            scalar_type,
            left,
            right,
            count_type: Some(count_type),
            kind,
        });
    }
    let (source, result, scalar_type, left, right, kind) = match operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactAdd,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactSubtract,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactMultiply,
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingAdd,
        ),
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingSubtract,
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingMultiply,
        ),
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingAdd,
        ),
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingSubtract,
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingMultiply,
        ),
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactDivide,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactRemainder,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingDivide,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingRemainder,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingDivide,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingRemainder,
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::BitwiseAnd,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::BitwiseOr,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::BitwiseXor,
        ),
        _ => return None,
    };
    Some(IntegerBinaryShape {
        source,
        result,
        scalar_type,
        left,
        right,
        count_type: None,
        kind,
    })
}

fn integer_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(IntegerValue, ScalarConstantFactIdentity)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine && fact.value == value)
            .then_some(fact)
            .and_then(|fact| match fact.constant {
                ScalarConstant::Integer(value) => fact.identity.map(|identity| (value, identity)),
                ScalarConstant::Boolean(_) => None,
            })
    })
}

fn boolean_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(bool, ScalarConstantFactIdentity)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine && fact.value == value)
            .then_some(fact)
            .and_then(|fact| match fact.constant {
                ScalarConstant::Boolean(value) => fact.identity.map(|identity| (value, identity)),
                ScalarConstant::Integer(_) => None,
            })
    })
}

fn integer_value_type(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    value: ValueId,
) -> Option<psi_core::IntegerType> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.definitions),
        )
        .find_map(|definition| {
            (definition.value == value)
                .then_some(definition.scalar_type)
                .and_then(|scalar_type| match scalar_type {
                    psi_core::ScalarType::Integer(integer) => Some(integer),
                    psi_core::ScalarType::Boolean => None,
                })
        })
}

fn propose_redundant_block_parameters(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ControlFlowGraph(_)) = analyses.get(AnalysisKind::ControlFlowGraph)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ControlFlowGraph,
        ));
    };
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

    let mut candidates = Vec::new();
    for function in &unit.functions {
        let machine_dominators = dominators
            .functions
            .iter()
            .find(|(machine, _)| *machine == function.machine)
            .map(|(_, rows)| rows.as_slice())
            .unwrap_or_default();
        for block in function
            .blocks
            .iter()
            .filter(|block| block.id != function.entry)
        {
            for (position, parameter) in block.parameters.iter().enumerate() {
                let mut incoming = Vec::new();
                for source in &function.blocks {
                    for node in &source.nodes {
                        for edge in &node.successors {
                            if edge.target != block.id {
                                continue;
                            }
                            let Some(binding) = edge.bindings.get(position) else {
                                continue;
                            };
                            incoming.push(BlockParameterIncomingBinding {
                                source: source.id,
                                edge: edge.psi_edge,
                                argument: binding.argument,
                            });
                        }
                    }
                }
                incoming.sort_by_key(|row| (row.edge, row.source));
                let Some(replacement) = incoming.first().map(|row| row.argument) else {
                    continue;
                };
                if replacement == parameter.value
                    || incoming.iter().any(|row| row.argument != replacement)
                    || !replacement_dominates_parameter_uses(
                        function.machine,
                        replacement,
                        parameter.value,
                        machine_dominators,
                        use_definitions,
                    )
                {
                    continue;
                }

                let mut affected_blocks = BTreeSet::from([block.id]);
                let mut provenance = Vec::new();
                for source in &function.blocks {
                    for (node_index, node) in source.nodes.iter().enumerate() {
                        let changes_use = node
                            .uses
                            .iter()
                            .any(|use_site| use_site.value == parameter.value);
                        for edge in node
                            .successors
                            .iter()
                            .filter(|edge| edge.target == block.id)
                        {
                            affected_blocks.insert(source.id);
                            let site = PsiRealizationSite::Edge {
                                machine: function.machine,
                                edge: edge.psi_edge,
                            };
                            provenance.push(ProvenanceRewrite {
                                input: site,
                                disposition: ProvenanceDisposition::RealizedAt(site),
                                sources: edge.provenance.clone(),
                                fuel: edge.fuel.clone(),
                            });
                        }
                        if changes_use {
                            affected_blocks.insert(source.id);
                            if !node.provenance.is_empty() {
                                let site = PsiRealizationSite::Node(NodeLocation {
                                    machine: function.machine,
                                    block: source.id,
                                    node: u32::try_from(node_index)
                                        .expect("unit node index fits u32"),
                                });
                                provenance.push(ProvenanceRewrite {
                                    input: site,
                                    disposition: ProvenanceDisposition::RealizedAt(site),
                                    sources: node.provenance.clone(),
                                    fuel: node.fuel.clone(),
                                });
                            }
                        }
                    }
                }
                provenance.sort_by_key(|row| {
                    (
                        row.input,
                        row.disposition.canonical_tag(),
                        row.disposition.site(),
                    )
                });
                candidates.push(
                    PsiRewriteCandidate::new_redundant_block_parameter(
                        unit.identity,
                        contract,
                        affected_blocks.into_iter().collect(),
                        provenance,
                        RedundantBlockParameterWitness { incoming },
                        -1,
                        RedundantBlockParameterRewrite {
                            machine: function.machine,
                            block: block.id,
                            position: u32::try_from(position)
                                .expect("unit parameter position fits u32"),
                            parameter: parameter.value,
                            replacement,
                            scalar_type: parameter.scalar_type,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

fn replacement_dominates_parameter_uses(
    machine: MachineId,
    replacement: ValueId,
    parameter: ValueId,
    dominators: &[(BlockId, Vec<BlockId>)],
    use_definitions: &crate::UseDefinitionAnalysis,
) -> bool {
    let Some((_, definition)) = use_definitions
        .definitions
        .iter()
        .find(|(owner, definition)| *owner == machine && definition.value == replacement)
    else {
        return false;
    };
    use_definitions
        .uses
        .iter()
        .filter(|(owner, use_site)| *owner == machine && use_site.value == parameter)
        .all(|(_, use_site)| match definition.site {
            omega_optimization_unit::ValueDefinitionSite::FunctionParameter(_) => true,
            omega_optimization_unit::ValueDefinitionSite::BlockParameter {
                block: defining,
                ..
            } => block_dominates(dominators, defining, use_site.block),
            omega_optimization_unit::ValueDefinitionSite::Node {
                block: defining,
                node,
            } if defining == use_site.block => node < use_site.node,
            omega_optimization_unit::ValueDefinitionSite::Node {
                block: defining, ..
            } => block_dominates(dominators, defining, use_site.block),
        })
}

fn block_dominates(
    dominators: &[(BlockId, Vec<BlockId>)],
    dominator: BlockId,
    block: BlockId,
) -> bool {
    dominators
        .iter()
        .find(|(candidate, _)| *candidate == block)
        .is_some_and(|(_, rows)| rows.contains(&dominator))
}

pub fn built_in_psi_registry(
    selections: &OptimizationSelections,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    let mut registries = built_in_psi_registries(selections)?;
    if registries.len() > 1 {
        return Err(RuleRegistryError::UnsupportedOptimizationCombination);
    }
    Ok(registries
        .pop()
        .unwrap_or_else(|| OrderedRuleRegistry::new(Vec::new()).expect("empty registry is valid")))
}

/// Build the canonical pass-group schedule for an exact named selection set.
///
/// Selection declaration order is not pass order. The explicit schedule below
/// runs semantic constant propagation before CFG and structural copy cleanup, and each
/// returned registry continues to own exactly one pass identity.
pub fn built_in_psi_registries(
    selections: &OptimizationSelections,
) -> Result<Vec<OrderedRuleRegistry>, RuleRegistryError> {
    let psi_selections = selections.for_phase(OptimizationExecutionPhase::Psi);
    if let Some(unsupported) = psi_selections.as_slice().iter().find(|optimization| {
        !matches!(
            optimization,
            Optimization::SparseConditionalConstantPropagation
                | Optimization::ControlFlowCleanup
                | Optimization::CopyPropagation
        )
    }) {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }
    let mut registries = Vec::new();
    if psi_selections.contains(Optimization::SparseConditionalConstantPropagation) {
        registries.push(registry_for_optimization(
            Optimization::SparseConditionalConstantPropagation,
        )?);
    }
    if psi_selections.contains(Optimization::ControlFlowCleanup) {
        registries.push(registry_for_optimization(Optimization::ControlFlowCleanup)?);
    }
    if psi_selections.contains(Optimization::CopyPropagation) {
        registries.push(registry_for_optimization(Optimization::CopyPropagation)?);
    }
    Ok(registries)
}

fn registry_for_optimization(
    optimization: Optimization,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    assemble_built_in_registry(built_in_rule_registrations(optimization))
}

#[derive(Debug, Clone)]
struct BuiltInRuleRegistration {
    schedule_ordinal: u16,
    rule: Arc<dyn PsiOptimizationRule>,
}

fn built_in_rule_registrations(optimization: Optimization) -> Vec<BuiltInRuleRegistration> {
    let mut registrations = Vec::new();
    macro_rules! register {
        ($ordinal:literal, $rule:expr) => {
            registrations.push(BuiltInRuleRegistration {
                schedule_ordinal: $ordinal,
                rule: Arc::new($rule),
            });
        };
    }
    if optimization == Optimization::SparseConditionalConstantPropagation {
        register!(0, ExactIntegerAddConstantsRule);
        register!(1, ExactIntegerSubtractConstantsRule);
        register!(2, ExactIntegerMultiplyConstantsRule);
        register!(3, WrappingIntegerAddConstantsRule);
        register!(4, WrappingIntegerSubtractConstantsRule);
        register!(5, WrappingIntegerMultiplyConstantsRule);
        register!(6, SaturatingIntegerAddConstantsRule);
        register!(7, SaturatingIntegerSubtractConstantsRule);
        register!(8, SaturatingIntegerMultiplyConstantsRule);
        register!(9, ExactIntegerDivideConstantsRule);
        register!(10, ExactIntegerRemainderConstantsRule);
        register!(11, WrappingIntegerDivideConstantsRule);
        register!(12, WrappingIntegerRemainderConstantsRule);
        register!(13, SaturatingIntegerDivideConstantsRule);
        register!(14, SaturatingIntegerRemainderConstantsRule);
        register!(15, ExactIntegerShiftLeftConstantsRule);
        register!(16, ExactIntegerShiftRightConstantsRule);
        register!(17, WrappingIntegerShiftLeftConstantsRule);
        register!(18, WrappingIntegerShiftRightConstantsRule);
        register!(19, ExactIntegerCastConstantsRule);
        register!(20, IntegerWidenConstantsRule);
        register!(21, IntegerBitwiseNotConstantsRule);
        register!(22, IntegerBitwiseAndConstantsRule);
        register!(23, IntegerBitwiseOrConstantsRule);
        register!(24, IntegerBitwiseXorConstantsRule);
        register!(25, BooleanNotConstantsRule);
        register!(26, BooleanEqualConstantsRule);
        register!(27, IntegerEqualConstantsRule);
        register!(28, IntegerLessThanConstantsRule);
        register!(29, IntegerLessOrEqualConstantsRule);
    }
    if optimization == Optimization::ControlFlowCleanup {
        register!(0, ConstantConditionalFoldRule);
        register!(1, LinearEmptyBlockThreadRule);
        register!(2, PathQualifiedEmptyBlockThreadRule);
    }
    if optimization == Optimization::CopyPropagation {
        register!(0, RedundantBlockParameterRule);
    }
    registrations
}

fn assemble_built_in_registry(
    mut registrations: Vec<BuiltInRuleRegistration>,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    registrations.sort_by_key(|registration| registration.schedule_ordinal);
    for (expected, registration) in registrations.iter().enumerate() {
        let expected = u16::try_from(expected).expect("built-in rule schedule fits u16");
        assert_eq!(
            registration.schedule_ordinal, expected,
            "built-in rule schedule ordinals must be unique and contiguous"
        );
    }
    OrderedRuleRegistry::new(
        registrations
            .into_iter()
            .map(|registration| registration.rule),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use omega_optimization_core::OptimizationValidatorIdentity;
    use omega_optimization_unit::{
        AcceptedObligationFact, OptimizationFact, PsiProvenance, PsiRewritePatch,
        attach_accepted_obligation_facts, recompute_psi_optimization_unit_identity,
        reconstruct_psi_optimization_unit_seed,
    };
    use omega_optimization_validation::{
        OptimizationUnitValidationError, validate_boolean_evaluation_candidate,
        validate_constant_conditional_candidate, validate_integer_evaluation_candidate,
        validate_linear_empty_block_candidate, validate_path_qualified_empty_block_candidate,
        validate_redundant_block_parameter_candidate,
    };
    use omega_terminal_abstract_operations::{
        TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractFunctionResult,
        TerminalAbstractOperation, TerminalAbstractOperationPlan, TerminalAbstractParameter,
        TerminalAbstractResult, TerminalAbstractSuccessor, TerminalValueBinding,
    };
    use psi_core::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ObligationId,
        OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;
    use crate::compute_analysis;

    fn shuffle_built_in_registrations(
        registrations: &mut [BuiltInRuleRegistration],
        mut state: u64,
    ) {
        for upper in (1..registrations.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let index = usize::try_from(
                state % u64::try_from(upper + 1).expect("registration count fits u64"),
            )
            .expect("shuffle index fits usize");
            registrations.swap(upper, index);
        }
    }

    pub(crate) fn randomized_sccp_registries() -> Vec<OrderedRuleRegistry> {
        (1..=32)
            .map(|seed| {
                let mut registrations =
                    built_in_rule_registrations(Optimization::SparseConditionalConstantPropagation);
                shuffle_built_in_registrations(&mut registrations, seed);
                assemble_built_in_registry(registrations)
                    .expect("shuffling cannot alter a valid built-in schedule")
            })
            .collect()
    }

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn with_synthetic_accepted_obligations(unit: PsiOptimizationUnit) -> PsiOptimizationUnit {
        let facts = unit
            .functions
            .iter()
            .flat_map(|function| {
                function.facts.iter().filter_map(|fact| match fact {
                    OptimizationFact::OperationObligationReference {
                        obligation,
                        support,
                    } => Some(AcceptedObligationFact::new(
                        unit.terminal_psi,
                        [29; 32],
                        function.machine,
                        *support,
                        *obligation,
                        obligation.get().to_le_bytes().to_vec(),
                    )),
                    _ => None,
                })
            })
            .collect();
        attach_accepted_obligation_facts(unit, facts).unwrap()
    }

    pub(crate) fn exact_add_unit() -> PsiOptimizationUnit {
        exact_chain_unit(false)
    }

    pub(crate) fn dependent_exact_chain_unit() -> PsiOptimizationUnit {
        exact_chain_unit(true)
    }

    fn exact_chain_unit(include_multiply: bool) -> PsiOptimizationUnit {
        let machine = id(301, MachineId::new);
        let block = id(302, BlockId::new);
        let left = id(303, ValueId::new);
        let right = id(304, ValueId::new);
        let sum = id(305, ValueId::new);
        let product = id(311, ValueId::new);
        let result = if include_multiply { product } else { sum };
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let mut operations = vec![
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: id(306, OperationId::new),
                result: left,
                scalar_type: ScalarType::Integer(integer),
                value: IntegerValue::Unsigned(7),
            },
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: id(307, OperationId::new),
                result: right,
                scalar_type: ScalarType::Integer(integer),
                value: IntegerValue::Unsigned(8),
            },
            TerminalAbstractOperation::ExactIntegerAdd {
                psi_operation: id(308, OperationId::new),
                obligation: id(309, ObligationId::new),
                result: sum,
                scalar_type: integer,
                left,
                right,
            },
        ];
        if include_multiply {
            operations.push(TerminalAbstractOperation::ExactIntegerMultiply {
                psi_operation: id(312, OperationId::new),
                obligation: id(313, ObligationId::new),
                result: product,
                scalar_type: integer,
                left: sum,
                right,
            });
        }
        operations.push(TerminalAbstractOperation::Return {
            psi_edge: id(310, EdgeId::new),
            result,
            value: result,
            scalar_type: ScalarType::Integer(integer),
            cleanup_actions: Vec::new(),
        });
        let unit = reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([13; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations,
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        with_synthetic_accepted_obligations(unit)
    }

    pub(crate) fn propagated_block_parameter_unit(constant: bool) -> PsiOptimizationUnit {
        let machine = id(601, MachineId::new);
        let entry = id(602, BlockId::new);
        let when_true = id(603, BlockId::new);
        let when_false = id(604, BlockId::new);
        let merge = id(605, BlockId::new);
        let condition = id(606, ValueId::new);
        let true_value = id(607, ValueId::new);
        let false_value = id(608, ValueId::new);
        let parameter = id(609, ValueId::new);
        let result = id(610, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let binding = |argument| TerminalValueBinding {
            parameter,
            argument,
            scalar_type,
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([21; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        TerminalAbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        TerminalAbstractBlockEntry {
                            block: when_true,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                        TerminalAbstractBlockEntry {
                            block: when_false,
                            parameters: Vec::new(),
                            operation_offset: 4,
                        },
                        TerminalAbstractBlockEntry {
                            block: merge,
                            parameters: vec![TerminalAbstractParameter {
                                value: parameter,
                                scalar_type,
                            }],
                            operation_offset: 6,
                        },
                    ],
                    operations: vec![
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: id(611, OperationId::new),
                            result: condition,
                            value: constant,
                        },
                        TerminalAbstractOperation::Conditional {
                            condition,
                            when_true: TerminalAbstractSuccessor {
                                psi_edge: id(612, EdgeId::new),
                                target: when_true,
                                bindings: Vec::new(),
                            },
                            when_false: TerminalAbstractSuccessor {
                                psi_edge: id(613, EdgeId::new),
                                target: when_false,
                                bindings: Vec::new(),
                            },
                        },
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(614, OperationId::new),
                            result: true_value,
                            scalar_type,
                            value: IntegerValue::Unsigned(7),
                        },
                        TerminalAbstractOperation::Jump {
                            psi_edge: id(615, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(true_value)],
                        },
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(616, OperationId::new),
                            result: false_value,
                            scalar_type,
                            value: IntegerValue::Unsigned(8),
                        },
                        TerminalAbstractOperation::Jump {
                            psi_edge: id(617, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(false_value)],
                        },
                        TerminalAbstractOperation::IntegerBitwiseNot {
                            psi_operation: id(618, OperationId::new),
                            result,
                            scalar_type: integer,
                            operand: parameter,
                        },
                        TerminalAbstractOperation::Return {
                            psi_edge: id(619, EdgeId::new),
                            result,
                            value: result,
                            scalar_type,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn linear_empty_block_unit() -> PsiOptimizationUnit {
        let machine = id(901, MachineId::new);
        let entry = id(902, BlockId::new);
        let empty = id(903, BlockId::new);
        let target = id(904, BlockId::new);
        let left = id(905, ValueId::new);
        let right = id(906, ValueId::new);
        let first = id(907, ValueId::new);
        let second = id(908, ValueId::new);
        let target_first = id(909, ValueId::new);
        let target_second = id(910, ValueId::new);
        let scalar_type = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 8).expect("valid fixture integer"),
        );
        let parameter = |value| TerminalAbstractParameter { value, scalar_type };
        let binding = |parameter, argument| TerminalValueBinding {
            parameter,
            argument,
            scalar_type,
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([31; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        TerminalAbstractParameter {
                            value: left,
                            scalar_type,
                        },
                        TerminalAbstractParameter {
                            value: right,
                            scalar_type,
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        TerminalAbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        TerminalAbstractBlockEntry {
                            block: empty,
                            parameters: vec![parameter(first), parameter(second)],
                            operation_offset: 1,
                        },
                        TerminalAbstractBlockEntry {
                            block: target,
                            parameters: vec![parameter(target_first), parameter(target_second)],
                            operation_offset: 2,
                        },
                    ],
                    operations: vec![
                        TerminalAbstractOperation::Jump {
                            psi_edge: id(911, EdgeId::new),
                            target: empty,
                            bindings: vec![binding(first, left), binding(second, right)],
                        },
                        TerminalAbstractOperation::Jump {
                            psi_edge: id(912, EdgeId::new),
                            target,
                            bindings: vec![
                                binding(target_first, second),
                                binding(target_second, first),
                            ],
                        },
                        TerminalAbstractOperation::ReturnUnit {
                            psi_edge: id(913, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn path_qualified_empty_block_unit() -> PsiOptimizationUnit {
        let machine = id(921, MachineId::new);
        let entry = id(922, BlockId::new);
        let left_block = id(923, BlockId::new);
        let right_block = id(924, BlockId::new);
        let empty = id(925, BlockId::new);
        let target = id(926, BlockId::new);
        let condition = id(927, ValueId::new);
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([32; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![TerminalAbstractParameter {
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    }],
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        TerminalAbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        TerminalAbstractBlockEntry {
                            block: left_block,
                            parameters: Vec::new(),
                            operation_offset: 1,
                        },
                        TerminalAbstractBlockEntry {
                            block: right_block,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                        TerminalAbstractBlockEntry {
                            block: empty,
                            parameters: Vec::new(),
                            operation_offset: 3,
                        },
                        TerminalAbstractBlockEntry {
                            block: target,
                            parameters: Vec::new(),
                            operation_offset: 4,
                        },
                    ],
                    operations: vec![
                        TerminalAbstractOperation::Conditional {
                            condition,
                            when_true: TerminalAbstractSuccessor {
                                psi_edge: id(931, EdgeId::new),
                                target: left_block,
                                bindings: Vec::new(),
                            },
                            when_false: TerminalAbstractSuccessor {
                                psi_edge: id(932, EdgeId::new),
                                target: right_block,
                                bindings: Vec::new(),
                            },
                        },
                        TerminalAbstractOperation::Jump {
                            psi_edge: id(933, EdgeId::new),
                            target: empty,
                            bindings: Vec::new(),
                        },
                        TerminalAbstractOperation::Jump {
                            psi_edge: id(934, EdgeId::new),
                            target: empty,
                            bindings: Vec::new(),
                        },
                        TerminalAbstractOperation::Jump {
                            psi_edge: id(935, EdgeId::new),
                            target,
                            bindings: Vec::new(),
                        },
                        TerminalAbstractOperation::ReturnUnit {
                            psi_edge: id(936, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn constant_conditional_same_target_unit(constant: bool) -> PsiOptimizationUnit {
        let machine = id(651, MachineId::new);
        let entry = id(652, BlockId::new);
        let merge = id(653, BlockId::new);
        let condition = id(654, ValueId::new);
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([23; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        TerminalAbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        TerminalAbstractBlockEntry {
                            block: merge,
                            parameters: Vec::new(),
                            operation_offset: 2,
                        },
                    ],
                    operations: vec![
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: id(655, OperationId::new),
                            result: condition,
                            value: constant,
                        },
                        TerminalAbstractOperation::Conditional {
                            condition,
                            when_true: TerminalAbstractSuccessor {
                                psi_edge: id(656, EdgeId::new),
                                target: merge,
                                bindings: Vec::new(),
                            },
                            when_false: TerminalAbstractSuccessor {
                                psi_edge: id(657, EdgeId::new),
                                target: merge,
                                bindings: Vec::new(),
                            },
                        },
                        TerminalAbstractOperation::ReturnUnit {
                            psi_edge: id(658, EdgeId::new),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn redundant_block_parameter_unit(redundant: bool) -> PsiOptimizationUnit {
        let machine = id(701, MachineId::new);
        let entry = id(702, BlockId::new);
        let merge = id(703, BlockId::new);
        let condition = id(704, ValueId::new);
        let shared = id(705, ValueId::new);
        let alternate = id(706, ValueId::new);
        let parameter = id(707, ValueId::new);
        let result = id(708, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let binding = |argument| TerminalValueBinding {
            parameter,
            argument,
            scalar_type,
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([22; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry,
                    parameters: vec![
                        TerminalAbstractParameter {
                            value: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        TerminalAbstractParameter {
                            value: shared,
                            scalar_type,
                        },
                        TerminalAbstractParameter {
                            value: alternate,
                            scalar_type,
                        },
                    ],
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![
                        TerminalAbstractBlockEntry {
                            block: entry,
                            parameters: Vec::new(),
                            operation_offset: 0,
                        },
                        TerminalAbstractBlockEntry {
                            block: merge,
                            parameters: vec![TerminalAbstractParameter {
                                value: parameter,
                                scalar_type,
                            }],
                            operation_offset: 1,
                        },
                    ],
                    operations: vec![
                        TerminalAbstractOperation::Conditional {
                            condition,
                            when_true: TerminalAbstractSuccessor {
                                psi_edge: id(709, EdgeId::new),
                                target: merge,
                                bindings: vec![binding(shared)],
                            },
                            when_false: TerminalAbstractSuccessor {
                                psi_edge: id(710, EdgeId::new),
                                target: merge,
                                bindings: vec![binding(if redundant { shared } else { alternate })],
                            },
                        },
                        TerminalAbstractOperation::ExactIntegerAdd {
                            psi_operation: id(711, OperationId::new),
                            obligation: id(713, ObligationId::new),
                            result,
                            scalar_type: integer,
                            left: parameter,
                            right: alternate,
                        },
                        TerminalAbstractOperation::Return {
                            psi_edge: id(712, EdgeId::new),
                            result,
                            value: result,
                            scalar_type,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    fn policy_add_unit(saturating: bool) -> PsiOptimizationUnit {
        let mut unit = exact_add_unit();
        let function = &mut unit.functions[0];
        let block = &mut function.blocks[0];
        let O::IntegerConstant { value, .. } = &mut block.nodes[0].operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(250);
        let O::IntegerConstant { value, .. } = &mut block.nodes[1].operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(10);
        let O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } = block.nodes[2].operation
        else {
            unreachable!()
        };
        block.nodes[2].operation = if saturating {
            O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
        } else {
            O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
        };
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[0] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(250);
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(10);
        function.facts.truncate(2);
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    pub(crate) fn wrapping_add_unit() -> PsiOptimizationUnit {
        policy_add_unit(false)
    }

    #[derive(Clone, Copy)]
    enum BitwiseFixtureKind {
        And,
        Or,
        Xor,
    }

    fn bitwise_unit(kind: BitwiseFixtureKind) -> PsiOptimizationUnit {
        let mut unit = exact_add_unit();
        let function = &mut unit.functions[0];
        let block = &mut function.blocks[0];
        let O::IntegerConstant { value, .. } = &mut block.nodes[0].operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(0b1010);
        let O::IntegerConstant { value, .. } = &mut block.nodes[1].operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(0b1100);
        let O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } = block.nodes[2].operation
        else {
            unreachable!()
        };
        block.nodes[2].operation = match kind {
            BitwiseFixtureKind::And => O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            BitwiseFixtureKind::Or => O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
            BitwiseFixtureKind::Xor => O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            },
        };
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[0] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(0b1010);
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(0b1100);
        function.facts.truncate(2);
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    #[derive(Clone, Copy)]
    enum ShiftFixtureKind {
        ExactLeft,
        ExactRight,
        WrappingLeft,
        WrappingRight,
    }

    fn shift_unit(kind: ShiftFixtureKind, value: u128, count: u128) -> PsiOptimizationUnit {
        let mut unit = exact_add_unit();
        let function = &mut unit.functions[0];
        let block = &mut function.blocks[0];
        let O::IntegerConstant {
            value: left_value, ..
        } = &mut block.nodes[0].operation
        else {
            unreachable!()
        };
        *left_value = IntegerValue::Unsigned(value);
        let O::IntegerConstant {
            value: right_value, ..
        } = &mut block.nodes[1].operation
        else {
            unreachable!()
        };
        *right_value = IntegerValue::Unsigned(count);
        let O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } = block.nodes[2].operation
        else {
            unreachable!()
        };
        block.nodes[2].operation = match kind {
            ShiftFixtureKind::ExactLeft => O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
            ShiftFixtureKind::ExactRight => O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
            ShiftFixtureKind::WrappingLeft => O::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
            ShiftFixtureKind::WrappingRight => O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type: scalar_type,
                count_type: scalar_type,
                value: left,
                count: right,
            },
        };
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[0] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(value);
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(count);
        if matches!(
            kind,
            ShiftFixtureKind::WrappingLeft | ShiftFixtureKind::WrappingRight
        ) {
            function.facts.truncate(2);
        }
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    fn exact_divide_unit(zero_divisor: bool) -> PsiOptimizationUnit {
        let mut unit = exact_add_unit();
        let function = &mut unit.functions[0];
        let block = &mut function.blocks[0];
        let O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } = block.nodes[2].operation
        else {
            unreachable!()
        };
        block.nodes[2].operation = O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        };
        if zero_divisor {
            let O::IntegerConstant { value, .. } = &mut block.nodes[1].operation else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(0);
            let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
                unreachable!()
            };
            *constant = IntegerValue::Unsigned(0);
        }
        unit.identity = recompute_psi_optimization_unit_identity(&unit);
        unit
    }

    fn exact_cast_unit(value: u128) -> PsiOptimizationUnit {
        let machine = id(321, MachineId::new);
        let block = id(322, BlockId::new);
        let operand = id(323, ValueId::new);
        let result = id(324, ValueId::new);
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let unit = reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([14; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(target_type),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(325, OperationId::new),
                            result: operand,
                            scalar_type: ScalarType::Integer(source_type),
                            value: IntegerValue::Unsigned(value),
                        },
                        TerminalAbstractOperation::IntegerExactCast {
                            psi_operation: id(326, OperationId::new),
                            obligation: id(327, ObligationId::new),
                            result,
                            source_type,
                            target_type,
                            operand,
                        },
                        TerminalAbstractOperation::Return {
                            psi_edge: id(328, EdgeId::new),
                            result,
                            value: result,
                            scalar_type: ScalarType::Integer(target_type),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        with_synthetic_accepted_obligations(unit)
    }

    fn goal_free_unary_unit(widen: bool) -> PsiOptimizationUnit {
        let machine = id(331, MachineId::new);
        let block = id(332, BlockId::new);
        let operand = id(333, ValueId::new);
        let result = id(334, ValueId::new);
        let source_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let target_type = if widen {
            IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
        } else {
            source_type
        };
        let unary = if widen {
            TerminalAbstractOperation::IntegerWiden {
                psi_operation: id(336, OperationId::new),
                result,
                source_type,
                target_type,
                operand,
            }
        } else {
            TerminalAbstractOperation::IntegerBitwiseNot {
                psi_operation: id(336, OperationId::new),
                result,
                scalar_type: source_type,
                operand,
            }
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([15; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(target_type),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(335, OperationId::new),
                            result: operand,
                            scalar_type: ScalarType::Integer(source_type),
                            value: IntegerValue::Unsigned(15),
                        },
                        unary,
                        TerminalAbstractOperation::Return {
                            psi_edge: id(337, EdgeId::new),
                            result,
                            value: result,
                            scalar_type: ScalarType::Integer(target_type),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    pub(crate) fn boolean_unit(equal: bool) -> PsiOptimizationUnit {
        let machine = id(341, MachineId::new);
        let block = id(342, BlockId::new);
        let left = id(343, ValueId::new);
        let right = id(344, ValueId::new);
        let result = id(345, ValueId::new);
        let operation = if equal {
            TerminalAbstractOperation::BooleanEqual {
                psi_operation: id(348, OperationId::new),
                result,
                left,
                right,
            }
        } else {
            TerminalAbstractOperation::BooleanNot {
                psi_operation: id(348, OperationId::new),
                result,
                operand: left,
            }
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([16; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Boolean,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: id(346, OperationId::new),
                            result: left,
                            value: true,
                        },
                        TerminalAbstractOperation::BooleanConstant {
                            psi_operation: id(347, OperationId::new),
                            result: right,
                            value: false,
                        },
                        operation,
                        TerminalAbstractOperation::Return {
                            psi_edge: id(349, EdgeId::new),
                            result,
                            value: result,
                            scalar_type: ScalarType::Boolean,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    #[derive(Clone, Copy)]
    enum ComparisonFixtureKind {
        Equal,
        LessThan,
        LessOrEqual,
    }

    fn integer_comparison_unit(kind: ComparisonFixtureKind) -> PsiOptimizationUnit {
        let machine = id(351, MachineId::new);
        let block = id(352, BlockId::new);
        let left = id(353, ValueId::new);
        let right = id(354, ValueId::new);
        let result = id(355, ValueId::new);
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let operation = match kind {
            ComparisonFixtureKind::Equal => TerminalAbstractOperation::IntegerEqual {
                psi_operation: id(358, OperationId::new),
                result,
                left,
                right,
            },
            ComparisonFixtureKind::LessThan => TerminalAbstractOperation::IntegerLessThan {
                psi_operation: id(358, OperationId::new),
                result,
                left,
                right,
            },
            ComparisonFixtureKind::LessOrEqual => TerminalAbstractOperation::IntegerLessOrEqual {
                psi_operation: id(358, OperationId::new),
                result,
                left,
                right,
            },
        };
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([17; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Boolean,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(356, OperationId::new),
                            result: left,
                            scalar_type: ScalarType::Integer(scalar_type),
                            value: IntegerValue::Unsigned(7),
                        },
                        TerminalAbstractOperation::IntegerConstant {
                            psi_operation: id(357, OperationId::new),
                            result: right,
                            scalar_type: ScalarType::Integer(scalar_type),
                            value: IntegerValue::Unsigned(8),
                        },
                        operation,
                        TerminalAbstractOperation::Return {
                            psi_edge: id(359, EdgeId::new),
                            result,
                            value: result,
                            scalar_type: ScalarType::Boolean,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn selected_builtin_proposes_one_independently_validated_exact_fold() {
        let unit = exact_add_unit();
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let products = vec![constants];
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        assert_eq!(registry.len(), 30);
        let mut dispatched = 0usize;
        let mut candidates = Vec::new();
        for rule in registry.iter() {
            dispatched += 1;
            candidates.extend(
                rule.propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap(),
            );
        }
        assert_eq!(dispatched, registry.len());
        assert_eq!(candidates.len(), 1);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(15),
                ..
            }
        ));
    }

    #[test]
    fn wrapping_and_saturating_rules_use_their_exact_declared_policies() {
        for (unit, saturating) in [(wrapping_add_unit(), false), (policy_add_unit(true), true)] {
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let products = vec![constants];
            let candidates = if saturating {
                SaturatingIntegerAddConstantsRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
            } else {
                WrappingIntegerAddConstantsRule
                    .propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap()
            };
            assert_eq!(candidates.len(), 1);
            assert_eq!(
                candidates[0].safety_class(),
                OptimizationSafetyClass::ExactOperationSemantics
            );
            let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
            let expected = if saturating { 255 } else { 4 };
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                TerminalAbstractOperation::IntegerConstant {
                    value: IntegerValue::Unsigned(value),
                    ..
                } if value == expected
            ));
        }
    }

    #[test]
    fn binary_bitwise_rules_fold_with_typed_psi_semantics() {
        let cases: [(BitwiseFixtureKind, &dyn PsiOptimizationRule, u128); 3] = [
            (BitwiseFixtureKind::And, &IntegerBitwiseAndConstantsRule, 8),
            (BitwiseFixtureKind::Or, &IntegerBitwiseOrConstantsRule, 14),
            (BitwiseFixtureKind::Xor, &IntegerBitwiseXorConstantsRule, 6),
        ];
        for (kind, rule, expected) in cases {
            let unit = bitwise_unit(kind);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            assert_eq!(
                candidates[0].safety_class(),
                OptimizationSafetyClass::ExactOperationSemantics
            );
            assert!(matches!(
                candidates[0].scalar_evaluation_witness().unwrap(),
                IntegerEvaluationWitness::Binary { .. }
            ));
            let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                TerminalAbstractOperation::IntegerConstant {
                    value: IntegerValue::Unsigned(value),
                    ..
                } if value == expected
            ));
        }
    }

    #[test]
    fn propagated_block_parameter_fact_is_independently_reconstructed() {
        let unit = propagated_block_parameter_unit(true);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = IntegerBitwiseNotConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[3].nodes[0].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(248),
                ..
            }
        ));
    }

    #[test]
    fn proof_bearing_division_folds_only_when_the_declared_operation_is_defined() {
        let unit = exact_divide_unit(false);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = ExactIntegerDivideConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(0),
                ..
            }
        ));

        let zero = exact_divide_unit(true);
        let constants = compute_analysis(&zero, AnalysisKind::ScalarConstants).unwrap();
        assert!(
            ExactIntegerDivideConstantsRule
                .propose(&zero, RuleAnalysisView::new(&[constants]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn exact_and_wrapping_shift_rules_use_psi_integer_semantics() {
        let cases: [(ShiftFixtureKind, &dyn PsiOptimizationRule, u128, u128, u128); 4] = [
            (
                ShiftFixtureKind::ExactLeft,
                &ExactIntegerShiftLeftConstantsRule,
                7,
                2,
                28,
            ),
            (
                ShiftFixtureKind::ExactRight,
                &ExactIntegerShiftRightConstantsRule,
                7,
                2,
                1,
            ),
            (
                ShiftFixtureKind::WrappingLeft,
                &WrappingIntegerShiftLeftConstantsRule,
                250,
                2,
                232,
            ),
            (
                ShiftFixtureKind::WrappingRight,
                &WrappingIntegerShiftRightConstantsRule,
                250,
                2,
                62,
            ),
        ];
        for (kind, rule, value, count, expected) in cases {
            let unit = shift_unit(kind, value, count);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            let expected_safety = if matches!(
                kind,
                ShiftFixtureKind::ExactLeft | ShiftFixtureKind::ExactRight
            ) {
                OptimizationSafetyClass::ProofCertified
            } else {
                OptimizationSafetyClass::ExactOperationSemantics
            };
            assert_eq!(candidates[0].safety_class(), expected_safety);
            let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                TerminalAbstractOperation::IntegerConstant {
                    value: IntegerValue::Unsigned(value),
                    ..
                } if value == expected
            ));
        }
    }

    #[test]
    fn exact_shift_left_declines_an_overflowing_constant_evaluation() {
        let unit = shift_unit(ShiftFixtureKind::ExactLeft, 250, 2);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        assert!(
            ExactIntegerShiftLeftConstantsRule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn exact_cast_rule_uses_unary_evidence_and_target_integer_semantics() {
        let unit = exact_cast_unit(250);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = ExactIntegerCastConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].safety_class(),
            OptimizationSafetyClass::ProofCertified
        );
        assert!(matches!(
            candidates[0].scalar_evaluation_witness().unwrap(),
            IntegerEvaluationWitness::ProofCertifiedUnary { .. }
        ));
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            TerminalAbstractOperation::IntegerConstant {
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(250),
                ..
            } if scalar_type == target_type
        ));

        let IntegerEvaluationWitness::ProofCertifiedUnary {
            operand_fact,
            obligation_fact,
        } = candidates[0].scalar_evaluation_witness().unwrap()
        else {
            unreachable!()
        };
        let omega_optimization_unit::PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) =
            candidates[0].patch()
        else {
            unreachable!()
        };
        let binary_witness = PsiRewriteCandidate::new_integer_evaluation(
            unit.identity,
            ExactIntegerCastConstantsRule::contract(),
            vec![unit.functions[0].blocks[0].id],
            Vec::new(),
            candidates[0].provenance().to_vec(),
            IntegerEvaluationWitness::ProofCertifiedBinary {
                left_fact: operand_fact,
                right_fact: operand_fact,
                obligation_fact,
            },
            -1,
            patch,
        )
        .unwrap();
        assert_eq!(binary_witness.consumed_facts().len(), 2);
        assert_ne!(binary_witness.identity(), candidates[0].identity());
        assert!(matches!(
            validate_integer_evaluation_candidate(&unit, &binary_witness),
            Err(omega_optimization_validation::OptimizationUnitValidationError::CandidateOperandFactMismatch)
        ));
    }

    #[test]
    fn exact_cast_rule_declines_a_constant_outside_the_target_domain() {
        let unit = exact_cast_unit(300);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        assert!(
            ExactIntegerCastConstantsRule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn widen_and_bitwise_not_rules_reuse_typed_unary_evidence() {
        let cases: [(bool, &dyn PsiOptimizationRule, u128, u16); 2] = [
            (true, &IntegerWidenConstantsRule, 15, 16),
            (false, &IntegerBitwiseNotConstantsRule, 240, 8),
        ];
        for (widen, rule, expected, expected_bits) in cases {
            let unit = goal_free_unary_unit(widen);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            assert_eq!(
                candidates[0].safety_class(),
                OptimizationSafetyClass::ExactOperationSemantics
            );
            assert!(matches!(
                candidates[0].scalar_evaluation_witness().unwrap(),
                IntegerEvaluationWitness::Unary { .. }
            ));
            let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[1].operation,
                TerminalAbstractOperation::IntegerConstant {
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(value),
                    ..
                } if value == expected && scalar_type.bits() == expected_bits
            ));
        }
    }

    #[test]
    fn boolean_not_and_equal_use_typed_boolean_patches() {
        let cases: [(bool, &dyn PsiOptimizationRule); 2] = [
            (false, &BooleanNotConstantsRule),
            (true, &BooleanEqualConstantsRule),
        ];
        for (equal, rule) in cases {
            let unit = boolean_unit(equal);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            assert!(matches!(
                candidates[0].patch(),
                omega_optimization_unit::PsiRewritePatch::ReplaceBooleanOperationWithConstant(_)
            ));
            let accepted = validate_boolean_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                TerminalAbstractOperation::BooleanConstant { value: false, .. }
            ));
        }
    }

    #[test]
    fn integer_comparison_rules_reconstruct_operand_types_and_boolean_results() {
        let cases: [(ComparisonFixtureKind, &dyn PsiOptimizationRule, bool); 3] = [
            (
                ComparisonFixtureKind::Equal,
                &IntegerEqualConstantsRule,
                false,
            ),
            (
                ComparisonFixtureKind::LessThan,
                &IntegerLessThanConstantsRule,
                true,
            ),
            (
                ComparisonFixtureKind::LessOrEqual,
                &IntegerLessOrEqualConstantsRule,
                true,
            ),
        ];
        for (kind, rule, expected) in cases {
            let unit = integer_comparison_unit(kind);
            let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&[constants]))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            let accepted = validate_boolean_evaluation_candidate(&unit, &candidates[0]).unwrap();
            assert!(matches!(
                accepted.unit().functions[0].blocks[0].nodes[2].operation,
                TerminalAbstractOperation::BooleanConstant { value, .. } if value == expected
            ));
        }
    }

    #[test]
    fn built_in_schedule_is_independent_of_registration_arrival_order() {
        let expected =
            registry_for_optimization(Optimization::SparseConditionalConstantPropagation).unwrap();
        let expected_contracts = expected.contracts().collect::<Vec<_>>();

        for registry in randomized_sccp_registries() {
            assert_eq!(registry.identity(), expected.identity());
            assert_eq!(registry.contracts().collect::<Vec<_>>(), expected_contracts);
        }
    }

    #[test]
    fn absent_selection_registers_nothing_and_missing_analysis_fails_closed() {
        let unit = exact_add_unit();
        assert!(
            built_in_psi_registry(&OptimizationSelections::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            ExactIntegerAddConstantsRule.propose(&unit, RuleAnalysisView::new(&[])),
            Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ScalarConstants
            ))
        );
        let cleanup = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        assert_eq!(built_in_psi_registry(&cleanup).unwrap().len(), 3);
        let copy = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        assert_eq!(built_in_psi_registry(&copy).unwrap().len(), 1);
        let unsupported_combination = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        assert!(matches!(
            built_in_psi_registry(&unsupported_combination),
            Err(RuleRegistryError::UnsupportedOptimizationCombination)
        ));

        let lower_only =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap();
        assert!(built_in_psi_registry(&lower_only).unwrap().is_empty());
        assert!(built_in_psi_registries(&lower_only).unwrap().is_empty());

        let sccp =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let mixed = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let sccp_registries = built_in_psi_registries(&sccp).unwrap();
        let mixed_registries = built_in_psi_registries(&mixed).unwrap();
        assert_eq!(mixed_registries.len(), 1);
        assert_eq!(
            mixed_registries[0].identity(),
            sccp_registries[0].identity()
        );
        assert_eq!(
            mixed_registries[0].contracts().collect::<Vec<_>>(),
            sccp_registries[0].contracts().collect::<Vec<_>>()
        );
    }

    #[test]
    fn constant_conditional_fold_binds_selected_edge_fact_and_fuel() {
        for constant in [false, true] {
            let unit = constant_conditional_same_target_unit(constant);
            let contract = ConstantConditionalFoldRule::contract();
            let mut manager = crate::AnalysisManager::new(&unit);
            let products = manager
                .require_all(&unit, contract.required_analyses())
                .unwrap()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let candidates = ConstantConditionalFoldRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap();
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].consumed_facts().len(), 1);
            let omega_optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
                candidates[0].patch()
            else {
                unreachable!()
            };
            assert_eq!(patch.constant, constant);
            let realized = candidates[0]
                .provenance()
                .iter()
                .find(|row| row.disposition.is_realized())
                .expect("conditional fold carries selected-edge custody");
            let proven_unreachable = candidates[0]
                .provenance()
                .iter()
                .find(|row| !row.disposition.is_realized())
                .expect("conditional fold carries rejected-edge custody");
            let realized_site = PsiRealizationSite::Edge {
                machine: patch.location.machine,
                edge: patch.selected_edge,
            };
            let unreachable_site = PsiRealizationSite::Edge {
                machine: patch.location.machine,
                edge: patch.rejected_edge,
            };
            assert_eq!(
                realized.disposition,
                ProvenanceDisposition::RealizedAt(realized_site)
            );
            assert_eq!(
                realized.sources,
                [omega_optimization_unit::PsiProvenance::Edge(
                    patch.selected_edge
                )]
            );
            assert_eq!(
                proven_unreachable.disposition,
                ProvenanceDisposition::ProvenUnreachableAt(unreachable_site)
            );
            assert_eq!(
                proven_unreachable.sources,
                [omega_optimization_unit::PsiProvenance::Edge(
                    patch.rejected_edge
                )]
            );
            let accepted = validate_constant_conditional_candidate(&unit, &candidates[0]).unwrap();
            assert_eq!(accepted.provenance(), candidates[0].provenance());
            assert_eq!(
                accepted.validator(),
                omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.constant-conditional-fold.v4"
                )
            );
            let node = &accepted.unit().functions[0].blocks[0].nodes[1];
            assert!(matches!(
                node.operation,
                TerminalAbstractOperation::Jump { psi_edge, .. } if psi_edge == patch.selected_edge
            ));
            assert_eq!(
                node.successors[0].provenance,
                [omega_optimization_unit::PsiProvenance::Edge(
                    patch.selected_edge
                )]
            );
            assert!(node.provenance.is_empty());
            assert!(node.fuel.is_empty());
            assert_eq!(node.successors[0].fuel.len(), 1);
            assert_eq!(
                node.successors[0].fuel[0].site,
                omega_optimization_unit::PsiProvenance::Edge(patch.selected_edge)
            );
        }
    }

    #[test]
    fn constant_conditional_fold_atomically_prunes_the_unreachable_branch_region() {
        let unit = propagated_block_parameter_unit(true);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .expect("constant branch produces an atomic prune candidate");
        assert_eq!(
            candidate.affected_blocks(),
            [
                id(602, BlockId::new),
                id(604, BlockId::new),
                id(605, BlockId::new),
            ]
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| row.disposition.is_realized())
                .count(),
            3
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| !row.disposition.is_realized())
                .count(),
            3
        );
        let accepted = validate_constant_conditional_candidate(&unit, &candidate).unwrap();
        let output = accepted.unit();
        assert_eq!(
            output.functions[0]
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            [
                id(602, BlockId::new),
                id(603, BlockId::new),
                id(605, BlockId::new),
            ]
        );
        assert_eq!(output.functions[0].facts.len(), 2);
        assert_eq!(output.functions[0].blocks[2].nodes[0].effect.input, 4);
        assert_eq!(output.functions[0].blocks[2].nodes[1].effect.output, 6);
        assert_eq!(accepted.provenance(), candidate.provenance());
    }

    #[test]
    fn constant_conditional_pruning_is_symmetric_and_rebases_all_later_blocks() {
        let unit = propagated_block_parameter_unit(false);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();

        assert_eq!(
            candidate.affected_blocks(),
            [
                id(602, BlockId::new),
                id(603, BlockId::new),
                id(604, BlockId::new),
                id(605, BlockId::new),
            ]
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| row.disposition.is_realized())
                .count(),
            4
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| !row.disposition.is_realized())
                .count(),
            3
        );
        let accepted = validate_constant_conditional_candidate(&unit, &candidate).unwrap();
        let output = accepted.unit();
        assert_eq!(
            output.functions[0]
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            [
                id(602, BlockId::new),
                id(604, BlockId::new),
                id(605, BlockId::new),
            ]
        );
        assert_eq!(output.functions[0].facts.len(), 2);
        assert_eq!(output.functions[0].blocks[1].nodes[0].effect.input, 2);
        assert_eq!(output.functions[0].blocks[2].nodes[1].effect.output, 6);
    }

    #[test]
    fn linear_empty_block_thread_composes_bindings_and_realizes_both_edges() {
        let unit = linear_empty_block_unit();
        let contract = LinearEmptyBlockThreadRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = LinearEmptyBlockThreadRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .expect("linear jump block is threadable");
        assert_eq!(
            candidate.affected_blocks(),
            [
                id(902, BlockId::new),
                id(903, BlockId::new),
                id(904, BlockId::new),
            ]
        );
        assert_eq!(candidate.provenance().len(), 3);
        assert!(
            candidate
                .provenance()
                .iter()
                .all(|row| row.disposition.is_realized())
        );
        assert_eq!(
            candidate
                .provenance()
                .iter()
                .filter(|row| {
                    matches!(row.input, PsiRealizationSite::Edge { .. })
                        && row.disposition.site()
                            == PsiRealizationSite::Edge {
                                machine: id(901, MachineId::new),
                                edge: id(911, psi_core::EdgeId::new),
                            }
                })
                .count(),
            2
        );

        let accepted = validate_linear_empty_block_candidate(&unit, &candidate).unwrap();
        assert_eq!(
            accepted.validator(),
            omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.linear-empty-block-thread.v2"
            )
        );
        let output = accepted.unit();
        assert_eq!(
            output.functions[0]
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            [id(902, BlockId::new), id(904, BlockId::new)]
        );
        let O::Jump {
            psi_edge,
            target,
            bindings,
        } = &output.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        assert_eq!(*psi_edge, id(911, EdgeId::new));
        assert_eq!(*target, id(904, BlockId::new));
        assert_eq!(bindings[0].argument, id(906, ValueId::new));
        assert_eq!(bindings[1].argument, id(905, ValueId::new));
        assert!(output.functions[0].blocks[0].nodes[0].provenance.is_empty());
        assert!(output.functions[0].blocks[0].nodes[0].fuel.is_empty());
        assert_eq!(
            output.functions[0].blocks[0].nodes[0].successors[0]
                .provenance
                .len(),
            2
        );
        assert_eq!(
            output.functions[0].blocks[0].nodes[0].successors[0]
                .fuel
                .len(),
            2
        );
        assert_eq!(output.functions[0].blocks[1].nodes[0].effect.input, 1);
        assert_eq!(output.functions[0].blocks[1].nodes[0].effect.output, 2);
    }

    #[test]
    fn linear_empty_block_validator_rejects_incomplete_fused_custody() {
        let unit = linear_empty_block_unit();
        let contract = LinearEmptyBlockThreadRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = LinearEmptyBlockThreadRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();
        let omega_optimization_unit::PsiRewritePatch::ThreadLinearEmptyBlock(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        let mut provenance = candidate.provenance().to_vec();
        let incoming = provenance
            .iter()
            .find(|row| {
                row.input
                    == PsiRealizationSite::Edge {
                        machine: patch.predecessor.machine,
                        edge: patch.incoming_edge,
                    }
            })
            .expect("incoming occurrence is present")
            .clone();
        let outgoing = provenance
            .iter_mut()
            .find(|row| {
                row.input
                    == PsiRealizationSite::Edge {
                        machine: patch.predecessor.machine,
                        edge: patch.outgoing_edge,
                    }
            })
            .expect("outgoing occurrence is present");
        outgoing.sources = incoming.sources;
        outgoing.fuel = incoming.fuel;
        let incomplete = PsiRewriteCandidate::new_linear_empty_block(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            provenance,
            -3,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_linear_empty_block_candidate(&unit, &incomplete),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        ));
    }

    #[test]
    fn path_qualified_empty_block_thread_fans_out_only_on_incoming_edge_antichain() {
        let unit = path_qualified_empty_block_unit();
        let contract = PathQualifiedEmptyBlockThreadRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = PathQualifiedEmptyBlockThreadRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .expect("two mutually exclusive incoming edges are threadable");
        let PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) = candidate.patch() else {
            unreachable!()
        };
        let outgoing_site = PsiRealizationSite::Edge {
            machine: patch.empty.machine,
            edge: patch.outgoing_edge,
        };
        let fanout = candidate
            .provenance()
            .iter()
            .filter(|row| row.input == outgoing_site)
            .collect::<Vec<_>>();
        assert_eq!(fanout.len(), 2);
        assert_ne!(fanout[0].disposition.site(), fanout[1].disposition.site());
        assert!(fanout.iter().all(|row| row.disposition.is_realized()));

        let accepted = validate_path_qualified_empty_block_candidate(&unit, &candidate).unwrap();
        assert_eq!(
            accepted.validator(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.path-qualified-empty-block-thread.v1"
            )
        );
        let function = &accepted.unit().functions[0];
        assert_eq!(function.blocks.len(), 4);
        assert!(
            !function
                .blocks
                .iter()
                .any(|block| block.id == patch.empty.block)
        );
        for edge_id in [id(933, EdgeId::new), id(934, EdgeId::new)] {
            let edge = function
                .blocks
                .iter()
                .flat_map(|block| block.nodes.iter())
                .flat_map(|node| node.successors.iter())
                .find(|edge| edge.psi_edge == edge_id)
                .expect("incoming edge survives");
            assert_eq!(edge.target, patch.target);
            assert_eq!(
                edge.provenance,
                [
                    PsiProvenance::Edge(edge_id),
                    PsiProvenance::Edge(patch.outgoing_edge),
                ]
            );
        }

        let mut coexecuted = accepted.unit().clone();
        let source = PsiProvenance::Edge(patch.outgoing_edge);
        coexecuted.functions[0].blocks[0].nodes[0].successors[0]
            .provenance
            .push(source);
        coexecuted.functions[0].blocks[0].nodes[0].successors[0]
            .fuel
            .push(omega_optimization_unit::FuelSettlement {
                site: source,
                units: 1,
            });
        coexecuted.identity = recompute_psi_optimization_unit_identity(&coexecuted);
        assert_eq!(
            omega_optimization_validation::validate_psi_optimization_unit(&coexecuted),
            Err(OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source))
        );
    }

    #[test]
    fn constant_conditional_validator_rejects_edge_and_fuel_corruption() {
        let unit = constant_conditional_same_target_unit(true);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();
        let omega_optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        let condition_fact = candidate
            .scalar_evaluation_witness()
            .and_then(IntegerEvaluationWitness::unary_operand)
            .unwrap();
        assert!(matches!(
            PsiRewriteCandidate::new_constant_conditional(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                candidate.provenance()[..1].to_vec(),
                condition_fact,
                -1,
                patch,
            ),
            Err(omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
        ));

        let mut duplicate_source = candidate.provenance().to_vec();
        let source = duplicate_source[0].sources[0];
        let fuel = duplicate_source[0].fuel[0];
        duplicate_source[0].sources.push(source);
        duplicate_source[0].fuel.push(fuel);
        assert!(matches!(
            PsiRewriteCandidate::new_constant_conditional(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                duplicate_source,
                condition_fact,
                -1,
                patch,
            ),
            Err(omega_optimization_unit::PsiRewriteCandidateError::NonCanonicalProvenance)
        ));

        let mut zero_fuel = candidate.provenance().to_vec();
        zero_fuel[1].fuel[0].units = 0;
        assert!(matches!(
            PsiRewriteCandidate::new_constant_conditional(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                zero_fuel,
                condition_fact,
                -1,
                patch,
            ),
            Err(omega_optimization_unit::PsiRewriteCandidateError::FuelProvenanceMismatch)
        ));

        let selected_site = PsiRealizationSite::Edge {
            machine: patch.location.machine,
            edge: patch.selected_edge,
        };
        let rejected_site = PsiRealizationSite::Edge {
            machine: patch.location.machine,
            edge: patch.rejected_edge,
        };
        let mut swapped_provenance = candidate.provenance().to_vec();
        for row in &mut swapped_provenance {
            if row.input == selected_site {
                row.disposition = ProvenanceDisposition::ProvenUnreachableAt(selected_site);
            } else if row.input == rejected_site {
                row.disposition = ProvenanceDisposition::RealizedAt(rejected_site);
            }
        }
        let swapped = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            swapped_provenance,
            condition_fact,
            -1,
            ConstantConditionalRewrite {
                selected_edge: patch.rejected_edge,
                rejected_edge: patch.selected_edge,
                ..patch
            },
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &swapped),
            Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
        ));

        let mut provenance = candidate.provenance().to_vec();
        provenance[0].fuel[0].units += 1;
        let wrong_fuel = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            provenance,
            condition_fact,
            -1,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &wrong_fuel),
            Err(OptimizationUnitValidationError::CandidateFuelMismatch)
        ));

        let mut provenance = candidate.provenance().to_vec();
        provenance[1].fuel[0].units += 1;
        let wrong_unreachable_fuel = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            provenance,
            condition_fact,
            -1,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &wrong_unreachable_fuel),
            Err(OptimizationUnitValidationError::CandidateFuelMismatch)
        ));
    }

    #[test]
    fn constant_conditional_validator_rejects_incomplete_prune_custody_and_region() {
        let unit = propagated_block_parameter_unit(true);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();
        let omega_optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        let condition_fact = candidate
            .scalar_evaluation_witness()
            .and_then(IntegerEvaluationWitness::unary_operand)
            .unwrap();
        let dead_block = id(604, BlockId::new);
        let rebased_merge = id(605, BlockId::new);

        let mut incomplete_provenance = candidate.provenance().to_vec();
        let removed = incomplete_provenance
            .iter()
            .position(|row| {
                !row.disposition.is_realized()
                    && matches!(
                        row.disposition.site(),
                        PsiRealizationSite::Node(location) if location.block == dead_block
                    )
            })
            .expect("dead nodes carry unreachable custody");
        incomplete_provenance.remove(removed);
        let incomplete_custody = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            incomplete_provenance,
            condition_fact,
            -1,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &incomplete_custody),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        ));

        let incomplete_region = PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate
                .affected_blocks()
                .iter()
                .copied()
                .filter(|block| *block != rebased_merge)
                .collect(),
            candidate.provenance().to_vec(),
            condition_fact,
            -1,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_constant_conditional_candidate(&unit, &incomplete_region),
            Err(OptimizationUnitValidationError::CandidateReachabilityMismatch)
        ));
    }

    #[test]
    fn redundant_block_parameter_rule_binds_both_exact_conditional_edges() {
        let unit = redundant_block_parameter_unit(true);
        let contract = RedundantBlockParameterRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidates = RedundantBlockParameterRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let witness = candidates[0].redundant_block_parameter_witness().unwrap();
        assert_eq!(witness.incoming.len(), 2);
        assert_eq!(witness.incoming[0].source, witness.incoming[1].source);
        assert_ne!(witness.incoming[0].edge, witness.incoming[1].edge);
        assert!(candidates[0].consumed_facts().is_empty());

        let accepted = validate_redundant_block_parameter_candidate(&unit, &candidates[0]).unwrap();
        assert_eq!(
            accepted.validator(),
            omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.redundant-block-parameter.v2"
            )
        );
        let output = accepted.unit();
        assert!(output.functions[0].blocks[1].parameters.is_empty());
        let O::Conditional {
            when_true,
            when_false,
            ..
        } = &output.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        assert!(when_true.bindings.is_empty());
        assert!(when_false.bindings.is_empty());
        let O::ExactIntegerAdd {
            obligation, left, ..
        } = output.functions[0].blocks[1].nodes[0].operation
        else {
            unreachable!()
        };
        assert_eq!(left, unit.functions[0].parameters[1].value);
        assert_eq!(obligation, id(713, ObligationId::new));
        assert_eq!(output.functions[0].facts, unit.functions[0].facts);
        for (before, after) in unit.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .zip(
                output.functions[0]
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes),
            )
        {
            assert_eq!(after.provenance, before.provenance);
            assert_eq!(after.fuel, before.fuel);
            assert_eq!(after.effect, before.effect);
            assert_eq!(after.ownership, before.ownership);
        }
    }

    #[test]
    fn differing_bindings_decline_and_incomplete_edge_witness_rejects() {
        let unit = redundant_block_parameter_unit(false);
        let contract = RedundantBlockParameterRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            RedundantBlockParameterRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        let unit = redundant_block_parameter_unit(true);
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = RedundantBlockParameterRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .pop()
            .unwrap();
        let omega_optimization_unit::PsiRewritePatch::RemoveRedundantBlockParameter(patch) =
            candidate.patch()
        else {
            unreachable!()
        };
        let incomplete = PsiRewriteCandidate::new_redundant_block_parameter(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance().to_vec(),
            RedundantBlockParameterWitness {
                incoming: candidate
                    .redundant_block_parameter_witness()
                    .unwrap()
                    .incoming[..1]
                    .to_vec(),
            },
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_ne!(incomplete.identity(), candidate.identity());
        assert_eq!(
            validate_redundant_block_parameter_candidate(&unit, &incomplete),
            Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
        );
    }
}
