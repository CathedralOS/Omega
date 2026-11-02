use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::CurrentSelectedProgram;

use physical_instructions::PostAllocationMachinePlan;

use crate::{OptimizedSelectedFormEncodingError, row_encoding::encode_row};
use machine_code::{
    SelectedFormEncodedRowStore, SelectedFormEncoding, SelectedFormEncodingIdentity,
};

pub(super) fn compute(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
) -> Result<SelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    crate::frame_address::validate_frame_root(machine, frame)?;
    if machine.selected != selected.identity {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    let target = selected.plan.target;
    // Reserve the machine plan's complete instruction roster once instead of
    // temporarily doubling a large row table. This is no coverage limit: the
    // same full paired walk below still produces and checks every instruction.
    let row_count = machine.instructions().count();
    let mut rows = Vec::with_capacity(row_count);
    // Every row's bytes and footprint land in these two buffers; a row keeps
    // only its span and handle into them.
    let mut encoded = SelectedFormEncodedRowStore::with_row_capacity(row_count);
    crate::rows::for_each_row(
        selected,
        machine,
        |selected_function, selected_instruction, machine_function, machine_instruction| {
            rows.push(encode_row(
                target,
                selected_instruction,
                machine_instruction,
                physical,
                selected_function,
                crate::frame_address::resolve(machine_function, frame, machine_instruction)?,
                &mut encoded,
            )?);
            Ok(())
        },
    )?;
    let counts = crate::rows::counts_of(&rows)?;
    let selected_root = selected.identity;
    let machine_root = machine.identity;
    let mut program = SelectedFormEncoding {
        selected: selected_root,
        machine: machine_root,
        post_allocation_machine_optimization: None,
        identity: SelectedFormEncodingIdentity::from_bytes([0; 32]),
        rows,
        encoded,
        frame: frame.cloned(),
        counts,
    };
    program.identity = program.recomputed_identity();
    Ok(program)
}
