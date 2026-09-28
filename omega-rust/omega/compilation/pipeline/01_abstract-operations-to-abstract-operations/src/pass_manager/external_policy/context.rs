use optimization_core::{
    ExternalDecisionContext, external_psi_decision_schema_v2_identity,
    psi_target_neutral_decision_target_v2_identity,
};
use optimization_core::{
    OptimizationRuleSetIdentity, OptimizationSelections, OptimizationUnitIdentity,
};

use super::super::{ExternalDecisionContextAxis, baseline_psi_cost_model_identity};

pub(crate) fn expected_context(
    source: OptimizationUnitIdentity,
    selections: &OptimizationSelections,
    phase_selections: &OptimizationSelections,
    rule_set: OptimizationRuleSetIdentity,
) -> ExternalDecisionContext {
    ExternalDecisionContext::new(
        external_psi_decision_schema_v2_identity(),
        source,
        selections.identity(),
        phase_selections.identity(),
        psi_target_neutral_decision_target_v2_identity(),
        rule_set,
        baseline_psi_cost_model_identity(),
    )
}

pub(super) fn mismatch(
    expected: ExternalDecisionContext,
    supplied: ExternalDecisionContext,
) -> Option<ExternalDecisionContextAxis> {
    if expected.schema() != supplied.schema() {
        Some(ExternalDecisionContextAxis::Schema)
    } else if expected.source() != supplied.source() {
        Some(ExternalDecisionContextAxis::Source)
    } else if expected.selections() != supplied.selections() {
        Some(ExternalDecisionContextAxis::Selections)
    } else if expected.phase_selections() != supplied.phase_selections() {
        Some(ExternalDecisionContextAxis::PhaseSelections)
    } else if expected.target() != supplied.target() {
        Some(ExternalDecisionContextAxis::Target)
    } else if expected.rule_set() != supplied.rule_set() {
        Some(ExternalDecisionContextAxis::RuleSet)
    } else if expected.cost_model() != supplied.cost_model() {
        Some(ExternalDecisionContextAxis::CostModel)
    } else {
        None
    }
}
