use std::collections::BTreeMap;

use selected_instructions::terminator_instruction;

use super::{
    super::AllocatedCalleeSavedRequirementError,
    state::{DirectTraversal, HomeIndex, add},
};

pub(super) fn derive(
    traversal: &mut DirectTraversal<'_>,
    selected: &selected_instructions::SelectedFunctions,
    homes: &[crate::FunctionRegisterHomes],
) -> Result<(), AllocatedCalleeSavedRequirementError> {
    if selected.len() != homes.len() {
        return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
    }
    for (function, homes) in selected.iter().zip(homes) {
        // Allocation replay already establishes the exact physical home roster;
        // dead semantic parameters remain selected without requiring a home.
        if function.machine != homes.machine {
            return Err(AllocatedCalleeSavedRequirementError::HomeRosterMismatch);
        }
        traversal.function_count = add(traversal.function_count, 1)?;
        let home_index = HomeIndex::new(homes);
        let mut units = BTreeMap::new();
        for block in &function.blocks {
            traversal.block_count = add(traversal.block_count, 1)?;
            for instruction in block
                .instructions
                .iter()
                .chain(std::iter::once(terminator_instruction(&block.terminator)))
            {
                traversal.scan_instruction(
                    function.machine,
                    block.id,
                    instruction,
                    &home_index,
                    &mut units,
                )?;
            }
        }
        traversal.finish(function.machine, units);
    }
    Ok(())
}
