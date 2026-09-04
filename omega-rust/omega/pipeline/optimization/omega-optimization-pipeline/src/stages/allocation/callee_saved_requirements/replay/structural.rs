use std::collections::BTreeMap;

use super::{
    super::{AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementError},
    state::{ReplayTraversal, add},
    writes::{scan_implicit, scan_instruction},
};

pub(super) fn reconstruct(
    traversal: &mut ReplayTraversal<'_>,
    function: &omega_selected_instructions::SelectedStructuralUnitFunction,
    homes: &omega_regalloc::FunctionRegisterHomes,
) -> Result<(), AllocatedCalleeSavedRequirementError> {
    if function.machine != homes.machine || !homes.assignments.is_empty() {
        return Err(AllocatedCalleeSavedRequirementError::HomeRosterMismatch);
    }
    traversal.function_count = add(traversal.function_count, 1)?;
    traversal.block_count = add(traversal.block_count, 1)?;
    let mut units = BTreeMap::new();
    if let Some(call) = &function.call {
        traversal.instruction_count = add(traversal.instruction_count, 1)?;
        scan_implicit(
            traversal,
            function.entry_block,
            call.id,
            &call.implicit_defs,
            &call.clobbers,
            &mut units,
        )?;
    }
    scan_instruction(
        traversal,
        function.machine,
        function.entry_block,
        &function.terminator.instruction,
        &BTreeMap::new(),
        &mut units,
    )?;
    traversal.finish(
        function.machine,
        AllocatedCalleeSavedFunctionKind::StructuralUnit,
        units,
    );
    Ok(())
}
