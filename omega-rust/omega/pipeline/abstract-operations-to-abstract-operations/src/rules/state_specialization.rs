//! State-argument specialization pass entrance and rule registration.

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    PsiOptimizationUnit, PsiRewriteCandidate, SpecializedStateEdgeRow,
    StateArgumentSpecializationRewrite,
};

use crate::rules::STATE_SPECIALIZATION_PASS_NAME;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError, state_specialization,
};

/// Binds the proven constant on an incoming edge — an unconditional `Jump`
/// successor or one arm of a `Conditional` predecessor — to a
/// `Conditional`-terminated dispatch block's parameter and fuses the resolved
/// arm, keeping both edges' provenance and fuel settlements on the fused
/// traversal. The condition reads the parameter directly (a Boolean
/// argument) or through an in-block `parameter CMP literal` integer
/// comparison against a pure scalar-constant prefix (an integer argument).
/// Machines holding an authenticated cyclic component stay frozen.
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
        let frozen = super::support::frozen_machines(unit, scc);
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
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![BuiltInRuleRegistration::new(
        0,
        StateArgumentSpecializationRule,
    )]
}
