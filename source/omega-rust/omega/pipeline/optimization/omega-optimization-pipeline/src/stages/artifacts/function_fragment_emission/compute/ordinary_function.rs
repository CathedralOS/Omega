mod block;
mod control;
mod instruction;

use omega_machine_code::FunctionFragment;
use omega_selected_instructions::SelectedFunction;

use crate::ResolvedSelectedFunctionLayout;

use super::super::FunctionFragmentEmissionError;

pub(super) fn emit(
    selected: &SelectedFunction,
    resolved: &ResolvedSelectedFunctionLayout,
) -> Result<FunctionFragment, FunctionFragmentEmissionError> {
    let mut bytes = Vec::new();
    let mut blocks = Vec::with_capacity(resolved.blocks.len());
    for resolved_block in &resolved.blocks {
        blocks.push(block::emit(selected, resolved_block, &mut bytes)?);
    }
    let byte_count =
        u64::try_from(bytes.len()).map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
    if byte_count != resolved.byte_count {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    Ok(FunctionFragment {
        machine: selected.machine,
        attachment: selected.attachment,
        provenance: selected.provenance.clone(),
        byte_count,
        bytes,
        blocks,
    })
}
