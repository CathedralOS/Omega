//! Optimizer module role: executable entrance. Copy propagation: the exact
//! redundant block-parameter rule and the pass's rule order.

use std::collections::BTreeSet;

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
    OptimizationSafetyClass,
};
use optimization_unit::{
    BlockParameterIncomingBinding, NodeLocation, PsiOptimizationUnit, PsiRealizationSite,
    PsiRewriteCandidate, RedundantBlockParameterRewrite, RedundantBlockParameterWitness,
};

use crate::rules::COPY_PROPAGATION_PASS_NAME;
use crate::rules::control_flow_cleanup::provenance_realized_at;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    RedundantBlockParameterRule,
    crate::rules::standard_rule_contract(
        b"omega.psi-rule.redundant-block-parameter.v1",
        COPY_PROPAGATION_PASS_NAME,
        1,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
        ]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries,]),
        OptimizationSafetyClass::StructuralIdentity
    ),
    propose = propose,
);

pub(super) fn propose(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    required_analysis!(analyses, AnalysisKind::ControlFlowGraph => Some(AnalysisProduct::ControlFlowGraph(_)));
    required_analysis!(analyses, AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators)));
    required_analysis!(analyses, AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions)));

    let mut candidates = Vec::new();
    for function in &unit.functions {
        let machine_dominators = dominators
            .function(function.machine)
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
                    || !use_definitions.replacement_dominates_parameter_uses(
                        function.machine,
                        replacement,
                        parameter.value,
                        machine_dominators,
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
                            provenance.push(provenance_realized_at(
                                site,
                                site,
                                &edge.provenance,
                                &edge.fuel,
                            ));
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
                                provenance.push(provenance_realized_at(
                                    site,
                                    site,
                                    &node.provenance,
                                    &node.fuel,
                                ));
                            }
                        }
                    }
                }
                provenance.sort_by_key(optimization_unit::ProvenanceRewrite::canonical_sort_key);
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
