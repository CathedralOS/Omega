use machine_code::{
    FunctionFragmentControlProvenance, FunctionFragmentInstructionSpan,
    StructuralUnitFunctionFragment, StructuralUnitFunctionFragmentBlockSpan,
};
use selected_instructions::SelectedStructuralUnitFunction;

use machine_code::ResolvedStructuralUnitFunctionLayout;

use super::call;
use crate::fragments::ResolvedFragmentEmissionError;

pub(in crate::fragments::production) fn emit(
    selected: &SelectedStructuralUnitFunction,
    resolved: &ResolvedStructuralUnitFunctionLayout,
) -> Result<StructuralUnitFunctionFragment, ResolvedFragmentEmissionError> {
    if selected.machine != resolved.machine
        || selected.entry_block != resolved.block
        || resolved.offset != 0
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let call = match (&selected.call, &resolved.call) {
        (None, None) => None,
        (Some(selected), Some(resolved)) => Some(call::emit(selected, resolved)?),
        _ => return Err(ResolvedFragmentEmissionError::RootMismatch),
    };
    let returned = &resolved.return_instruction;
    let selected_return = &selected.terminator.instruction;
    if selected_return.id != returned.instruction {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let return_instruction = FunctionFragmentInstructionSpan {
        instruction: returned.instruction,
        alternative: returned.alternative,
        offset: returned.offset,
        bytes: returned.bytes.clone(),
        branch: None,
        internal_machine_fixup: None,
        provenance: selected_return.provenance.clone(),
        control: FunctionFragmentControlProvenance::Return {
            psi_return_edge: selected.terminator.psi_return_edge,
        },
    };
    let mut bytes = Vec::new();
    if let Some(call) = &call {
        if u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?
            != call.offset
        {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        bytes.extend_from_slice(&call.bytes);
    }
    if u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?
        != return_instruction.offset
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    bytes.extend_from_slice(&return_instruction.bytes);
    if u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?
        != resolved.byte_count
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    Ok(StructuralUnitFunctionFragment {
        machine: selected.machine,
        attachment: selected.attachment,
        provenance: selected.provenance.clone(),
        byte_count: resolved.byte_count,
        bytes,
        block: StructuralUnitFunctionFragmentBlockSpan {
            block: resolved.block,
            offset: resolved.offset,
            byte_count: resolved.byte_count,
            call,
            return_instruction,
        },
    })
}
