use std::collections::BTreeMap;

use selected_instructions::{SelectedInstruction, SelectedTerminator};

use super::{
    super::{AllocatedCalleeSavedRequirementError, FunctionAllocatedCalleeSavedRequirements},
    state::{DirectTraversal, add, finish_units},
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
                    homes,
                    &mut units,
                )?;
            }
        }
        traversal
            .functions
            .push(FunctionAllocatedCalleeSavedRequirements {
                machine: function.machine,
                modified_units: finish_units(units),
            });
    }
    Ok(())
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
    }
}
