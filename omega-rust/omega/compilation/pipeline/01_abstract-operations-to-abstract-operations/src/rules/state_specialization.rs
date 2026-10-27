//! State-argument specialization pass entrance and rule registration.

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
    OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::rules::STATE_SPECIALIZATION_PASS_NAME;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError, state_specialization,
};

psi_rule_contract!(
    /// Binds the proven constant on an incoming edge — an unconditional `Jump`
    /// successor or one arm of a `Conditional` predecessor — to a
    /// `Conditional`-terminated dispatch block's parameter and fuses the resolved
    /// arm, keeping both edges' provenance and fuel settlements on the fused
    /// traversal. The condition reads the parameter directly (a Boolean
    /// argument) or through an in-block `parameter CMP literal` integer
    /// comparison against a pure scalar-constant prefix (an integer argument).
    /// Machines holding an authenticated cyclic component stay frozen.
    StateArgumentSpecializationRule,
    crate::rules::standard_rule_contract(
        b"omega.psi-rule.state-argument-specialization.v1",
        STATE_SPECIALIZATION_PASS_NAME,
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
        OptimizationSafetyClass::StructuralIdentity
    ),
);

psi_rule_impl!(StateArgumentSpecializationRule, |unit, analyses| {
    required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
    required_analysis!(analyses, AnalysisKind::StronglyConnectedComponents => Some(AnalysisProduct::StronglyConnectedComponents(scc)));
    // This specialization preserves cyclic machines byte-for-byte.
    let frozen = scc.cyclic_machines(unit);
    let mut candidates = Vec::new();
    for function in &unit.functions {
        if frozen.contains(&function.machine) {
            continue;
        }
        for block in &function.blocks {
            let Some(patch) =
                state_specialization::propose::plan(unit, function, block.id, constants)
            else {
                continue;
            };
            if patch.edges.is_empty() {
                continue;
            }
            let Some(provenance) =
                state_specialization::accounting::provenance_rows(function, &patch)
            else {
                continue;
            };
            let mut affected_blocks = vec![patch.dispatch];
            affected_blocks.extend(patch.edges.iter().map(|row| row.predecessor.block));
            affected_blocks.sort_unstable();
            affected_blocks.dedup();
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
});

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![BuiltInRuleRegistration::new(
        0,
        StateArgumentSpecializationRule,
    )]
}
