use register_model::RegisterInstructionConstraint;
use selected_instructions::SelectedInstructionPlan;

use crate::{
    PressureRematerializationError, PressureRematerializationPlan, ValidatedLiveRanges,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis,
};

use super::{application, decision, selected_structure};

pub(super) struct Replay {
    pub(super) transformed: SelectedInstructionPlan,
    pub(super) applied: usize,
    pub(super) rewritten_uses: usize,
}

pub(super) fn reconstruct(
    selected: &impl ValidatedSelectedAnalysis,
    ranges: &ValidatedLiveRanges,
    recovery: &ValidatedRecoveryClassifications,
    plan: &PressureRematerializationPlan,
    row: &RegisterInstructionConstraint,
) -> Result<Replay, PressureRematerializationError> {
    let mut transformed = selected.selected_plan().clone();
    let mut applied = 0usize;
    let mut rewritten_uses = 0usize;
    for index in 0..plan.functions.len() {
        let source = &selected.selected_plan().functions[index];
        let function_plan = &plan.functions[index];
        let range_function = &ranges.plan().functions[index];
        let recovery_function = &recovery.plan().functions[index];
        if source.machine != function_plan.machine
            || source.machine != range_function.machine
            || source.machine != recovery_function.machine
        {
            return Err(PressureRematerializationError::FunctionMismatch { function: index });
        }
        selected_structure::validate_dense(index, source)?;
        match (
            &recovery_function.classification,
            function_plan.action.as_ref(),
        ) {
            (None, None) => {}
            (Some(candidate), Some(action)) => {
                decision::validate(
                    index,
                    source,
                    range_function,
                    candidate,
                    action,
                    row,
                    plan.policy,
                )?;
                application::replay(index, &mut transformed.functions[index], action, row)?;
                applied = applied
                    .checked_add(1)
                    .ok_or(PressureRematerializationError::WorkOverflow)?;
                rewritten_uses = rewritten_uses
                    .checked_add(action.rewrites.len())
                    .ok_or(PressureRematerializationError::WorkOverflow)?;
            }
            _ => {
                return Err(PressureRematerializationError::DecisionMismatch { function: index });
            }
        }
    }
    if applied == 0 {
        return Err(PressureRematerializationError::NoAction);
    }
    Ok(Replay {
        transformed,
        applied,
        rewritten_uses,
    })
}
