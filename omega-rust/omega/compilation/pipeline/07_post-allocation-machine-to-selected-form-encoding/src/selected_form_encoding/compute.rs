use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use crate::{OptimizedSelectedFormEncodingError, row_encoding::encode_row};
use machine_code::{
    SelectedFormEncodedRowStore, SelectedFormEncoding, SelectedFormEncodingIdentity,
};

pub(super) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
) -> Result<SelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let machine = staged.machine().plan();
    crate::frame_address::validate_frame_root(machine, frame)?;
    if machine.selected != selected.selected_identity() {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    let target = selected.selected_plan().target;
    let mut rows = Vec::new();
    // Every row's bytes and footprint land in these two buffers; a row keeps
    // only its span and handle into them.
    let mut encoded = SelectedFormEncodedRowStore::new();
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
    let selected_root = selected.selected_identity();
    let machine_root = staged.machine().receipt().identity();
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
