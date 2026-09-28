//! Selected schedule reconstruction and canonical whole-run rule identity.

use crate::PsiPassStep;
use crate::rules::built_in_psi_steps_for_selections;
use optimization_core::{OptimizationRuleSetIdentity, OptimizationSelections};

use crate::OptimizedAbstractProjectionError;

pub(super) struct RebuiltRuleSchedule {
    pub(super) steps: Vec<PsiPassStep>,
    pub(super) ordered_rule_set: OptimizationRuleSetIdentity,
}

pub(super) fn rebuild(
    selections: &OptimizationSelections,
) -> Result<RebuiltRuleSchedule, OptimizedAbstractProjectionError> {
    let projection = selections.project_psi();
    let steps = built_in_psi_steps_for_selections(projection.selections())
        .map_err(OptimizedAbstractProjectionError::Registry)?;
    let ordered_rules = steps
        .iter()
        .flat_map(PsiPassStep::ordered_rules)
        .collect::<Vec<_>>();
    let ordered_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&ordered_rules)
        .map_err(|_| OptimizedAbstractProjectionError::CommitReplayMismatch)?;
    Ok(RebuiltRuleSchedule {
        steps,
        ordered_rule_set,
    })
}
