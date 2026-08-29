use std::collections::BTreeSet;

use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    BlockParameterIncomingBinding, NodeLocation, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate, RedundantBlockParameterRewrite,
    RedundantBlockParameterWitness,
};

use super::{COPY_PROPAGATION_PASS_NAME, support::replacement_dominates_parameter_uses};

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
