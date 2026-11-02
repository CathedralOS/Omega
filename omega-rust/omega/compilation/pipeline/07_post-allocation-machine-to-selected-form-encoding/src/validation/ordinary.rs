use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::CurrentSelectedProgram;

use physical_instructions::PostAllocationMachinePlan;

use super::{super::OptimizedSelectedFormEncodingError, row};
use machine_code::{SelectedFormEncodedRowStore, SelectedFormEncodingRow};

pub(super) fn validate(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
    rows: &[SelectedFormEncodingRow],
    encoded: &SelectedFormEncodedRowStore,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let target = selected.plan.target;
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
                candidate.address(),
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
