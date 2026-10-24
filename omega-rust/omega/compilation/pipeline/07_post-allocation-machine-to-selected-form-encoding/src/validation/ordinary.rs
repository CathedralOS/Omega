use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::{super::OptimizedSelectedFormEncodingError, row};
use machine_code::{SelectedFormEncodedRowStore, SelectedFormEncodingRow};

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
    rows: &[SelectedFormEncodingRow],
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let machine = staged.machine().plan();
    let target = selected.selected_plan().target;
    let mut candidate_rows = rows.iter();
    crate::rows::for_each_row(
        selected,
        machine,
        |selected_function, selected_instruction, machine_function, machine_instruction| {
            let candidate = candidate_rows
                .next()
                .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)?;
            crate::frame_address::validate_address(
                machine_function,
                frame,
                machine_instruction,
                candidate.address,
            )?;
            row::validate(
                target,
                selected_function,
                selected_instruction,
                machine_instruction,
                physical,
                candidate,
                encoded,
            )
        },
    )?;
    if candidate_rows.next().is_some() {
        return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
    }
    Ok(())
}
