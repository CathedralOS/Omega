//! State-argument specialization pass entrance and rule registration.

use std::collections::BTreeSet;

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    PsiOptimizationUnit, PsiRewriteCandidate, SpecializedStateEdgeRow,
    StateArgumentSpecializationRewrite,
};
use semantic_vocabulary::MachineId;

use crate::rules::catalog::BuiltInRuleRegistration;
use crate::state_specialization;
use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    StronglyConnectedComponentAnalysis,
};

use super::STATE_SPECIALIZATION_PASS_NAME;

/// Binds the proven Boolean constant on an incoming unconditional `Jump` edge
/// to a single-`Conditional` dispatch block's parameter and fuses the resolved
/// arm, keeping both edges' provenance and fuel settlements on the fused
/// traversal. Machines holding an authenticated cyclic component stay frozen.
#[derive(Debug, Clone, Copy, Default)]
pub struct StateArgumentSpecializationRule;

impl StateArgumentSpecializationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.state-argument-specialization.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(STATE_SPECIALIZATION_PASS_NAME),
            1,
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::StronglyConnectedComponents,
            ]),
            AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::PostDominators,
                AnalysisKind::LoopForest,
                AnalysisKind::StronglyConnectedComponents,
                AnalysisKind::UseDefinition,
                AnalysisKind::ExecutableEdges,
                AnalysisKind::ScalarConstants,
                AnalysisKind::ValueRanges,
                AnalysisKind::EffectSummaries,
                AnalysisKind::ValueLiveness,
            ]),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in rule has nonzero version")
    }

    /// Machines whose block projection contains a cyclic component —
    /// a multi-block SCC or a singleton self-loop — are frozen byte-exact,
    /// matching the bespoke family's cycle roster.
    fn frozen_machines(
        unit: &PsiOptimizationUnit,
        scc: &StronglyConnectedComponentAnalysis,
    ) -> BTreeSet<MachineId> {
        unit.functions
            .iter()
            .filter(|function| {
                let Some((_, components)) = scc
                    .functions
                    .iter()
                    .find(|(machine, _)| *machine == function.machine)
                else {
                    return false;
                };
                components.iter().any(|component| {
                    component.len() > 1
                        || component.iter().any(|block| {
                            function
                                .blocks
                                .iter()
                                .find(|candidate| candidate.id == *block)
                                .is_some_and(|owner| {
                                    owner
                                        .nodes
                                        .iter()
                                        .flat_map(|node| &node.successors)
                                        .any(|edge| edge.target == *block)
                                })
                        })
                })
            })
            .map(|function| function.machine)
            .collect()
    }
}

impl PsiOptimizationRule for StateArgumentSpecializationRule {
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
        let Some(AnalysisProduct::StronglyConnectedComponents(scc)) =
            analyses.get(AnalysisKind::StronglyConnectedComponents)
        else {
            return Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::StronglyConnectedComponents,
            ));
        };
        let frozen = Self::frozen_machines(unit, scc);
        let mut candidates = Vec::new();
        for function in &unit.functions {
            if frozen.contains(&function.machine) {
                continue;
            }
            for block in &function.blocks {
                let Some(plan) =
                    state_specialization::propose::plan(unit, function, block.id, constants)
                else {
                    continue;
                };
                if plan.edges.is_empty() {
                    continue;
                }
                let edges = plan
                    .edges
                    .iter()
                    .map(|edge| SpecializedStateEdgeRow {
                        incoming_edge: edge.incoming_edge(),
                        predecessor: edge.predecessor(),
                        parameter: edge.parameter(),
                        argument: edge.argument(),
                        constant: edge.constant(),
                        taken_edge: edge.taken_edge(),
                        rejected_edge: edge.rejected_edge(),
                        resolved_target: edge.resolved_target(),
                    })
                    .collect::<Vec<_>>();
                let Ok(provenance) =
                    state_specialization::validate::provenance_rows(function, plan.machine, &plan)
                else {
                    continue;
                };
                let mut affected_blocks = vec![plan.dispatch];
                affected_blocks.extend(edges.iter().map(|row| row.predecessor.block));
                affected_blocks.sort_unstable();
                affected_blocks.dedup();
                let patch = StateArgumentSpecializationRewrite {
                    machine: plan.machine,
                    dispatch: plan.dispatch,
                    edges,
                };
                let predicted_cost_delta = -i64::try_from(patch.edges.len()).unwrap_or(i64::MAX);
                candidates.push(
                    PsiRewriteCandidate::new_state_argument_specialization(
                        unit.identity,
                        Self::contract(),
                        affected_blocks,
                        provenance,
                        predicted_cost_delta,
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
        Ok(candidates)
    }
}

/// The exact local rule order for this pass.
pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![BuiltInRuleRegistration::new(
        0,
        StateArgumentSpecializationRule,
    )]
}
