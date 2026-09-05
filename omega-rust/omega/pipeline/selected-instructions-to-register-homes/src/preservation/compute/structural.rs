use std::collections::BTreeMap;

use super::{
    super::{
        AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementError,
        FunctionAllocatedCalleeSavedRequirements,
    },
    state::{DirectTraversal, add, finish_units},
};

pub(super) fn derive(
    traversal: &mut DirectTraversal<'_>,
    selected: &[selected_instructions::SelectedStructuralUnitFunction],
    homes: &[crate::FunctionRegisterHomes],
) -> Result<(), AllocatedCalleeSavedRequirementError> {
    if selected.len() != homes.len() {
        return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
    }
    for (function, homes) in selected.iter().zip(homes) {
        if function.machine != homes.machine || !homes.assignments.is_empty() {
            return Err(AllocatedCalleeSavedRequirementError::HomeRosterMismatch);
        }
        traversal.function_count = add(traversal.function_count, 1)?;
        traversal.block_count = add(traversal.block_count, 1)?;
        let mut units = BTreeMap::new();
        if let Some(call) = &function.call {
            traversal.scan_implicit(
                function.entry_block,
                call.id,
                &call.implicit_defs,
                &call.clobbers,
                &mut units,
            )?;
        }
        traversal.scan_instruction(
            function.machine,
            function.entry_block,
            &function.terminator.instruction,
            homes,
            &mut units,
        )?;
        traversal
            .functions
            .push(FunctionAllocatedCalleeSavedRequirements {
                machine: function.machine,
                kind: AllocatedCalleeSavedFunctionKind::StructuralUnit,
                modified_units: finish_units(units),
            });
    }
    Ok(())
}
