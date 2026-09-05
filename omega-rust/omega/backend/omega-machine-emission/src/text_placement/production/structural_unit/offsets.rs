use std::collections::BTreeMap;

use omega_machine_code::StructuralUnitFunctionFragment;
use psi_core::MachineId;

use super::super::super::TextPlacementError;

pub(super) struct FunctionOffsets {
    pub(super) by_machine: BTreeMap<MachineId, u64>,
    pub(super) section_byte_count: u64,
    pub(super) semantic_entry: u64,
}

pub(super) fn derive(
    functions: &[StructuralUnitFunctionFragment],
    entry: MachineId,
) -> Result<FunctionOffsets, TextPlacementError> {
    let mut by_machine = BTreeMap::new();
    let mut section_byte_count = 0_u64;
    let mut semantic_entry = None;
    for function in functions {
        if u64::try_from(function.bytes.len()).map_err(|_| TextPlacementError::OffsetOverflow)?
            != function.byte_count
        {
            return Err(TextPlacementError::SourceShapeMismatch);
        }
        if by_machine
            .insert(function.machine, section_byte_count)
            .is_some()
        {
            return Err(TextPlacementError::DuplicateFunction(function.machine));
        }
        if function.machine == entry && semantic_entry.replace(section_byte_count).is_some() {
            return Err(TextPlacementError::DuplicateSemanticEntry(entry));
        }
        section_byte_count = section_byte_count
            .checked_add(function.byte_count)
            .ok_or(TextPlacementError::OffsetOverflow)?;
    }
    let semantic_entry = semantic_entry.ok_or(TextPlacementError::MissingSemanticEntry(entry))?;
    Ok(FunctionOffsets {
        by_machine,
        section_byte_count,
        semantic_entry,
    })
}
