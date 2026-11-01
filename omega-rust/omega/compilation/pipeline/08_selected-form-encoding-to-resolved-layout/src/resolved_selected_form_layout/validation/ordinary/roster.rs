use std::collections::BTreeMap;

use selected_instructions::{SelectedFunction, SelectedInstructionId};

use machine_code::SelectedFormEncodingRow;

use super::super::super::OptimizedResolvedSelectedFormLayoutError as Error;
use super::PreLayoutRows;

pub(super) fn collect_pre_layout_rows<'a>(
    function: &SelectedFunction,
    rows: &mut PreLayoutRows<'a>,
) -> Result<BTreeMap<SelectedInstructionId, &'a SelectedFormEncodingRow>, Error> {
    let mut collected = BTreeMap::new();
    for block in &function.blocks {
        for instruction in crate::resolved_selected_form_layout::ordinary::instructions(block) {
            let row = rows
                .next()
                .ok_or(Error::MissingInstruction(instruction.id))?;
            if row.instruction != instruction.id {
                return Err(Error::MissingInstruction(instruction.id));
            }
            if collected.insert(instruction.id, row).is_some() {
                return Err(Error::DuplicateInstruction(instruction.id));
            }
        }
    }
    Ok(collected)
}
