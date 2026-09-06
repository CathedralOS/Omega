use std::collections::BTreeMap;

use selected_instructions::{SelectedInstruction, SelectedTerminator};

use super::{
    super::{AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementError},
    state::{ReplayTraversal, add},
    writes::{index_homes, scan_instruction},
};

pub(super) fn reconstruct(
    traversal: &mut ReplayTraversal<'_>,
    function: &selected_instructions::SelectedFunction,
    homes: &crate::FunctionRegisterHomes,
) -> Result<(), AllocatedCalleeSavedRequirementError> {
    if function.machine != homes.machine
        || function.virtual_registers.len() != homes.assignments.len()
    {
        return Err(AllocatedCalleeSavedRequirementError::HomeRosterMismatch);
    }
    let home_index = index_homes(homes)?;
    traversal.function_count = add(traversal.function_count, 1)?;
    let mut units = BTreeMap::new();
    for block in &function.blocks {
        traversal.block_count = add(traversal.block_count, 1)?;
        for instruction in &block.instructions {
            scan_instruction(
                traversal,
                function.machine,
                block.id,
                instruction,
                &home_index,
                &mut units,
            )?;
        }
        scan_instruction(
            traversal,
            function.machine,
            block.id,
            terminator_instruction(&block.terminator),
            &home_index,
            &mut units,
        )?;
    }
    traversal.finish(
        function.machine,
        AllocatedCalleeSavedFunctionKind::Ordinary,
        units,
    );
    Ok(())
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}
