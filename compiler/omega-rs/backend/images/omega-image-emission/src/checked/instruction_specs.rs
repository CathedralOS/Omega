//! Reconstructs exact expected bytes and relocation recipes for compiler instructions.

use super::*;

mod arithmetic_convert;
mod buffer_wire_text;
mod control_entry;
mod outbound_calls;
mod storage_place;

pub(super) type CompilerInstructionSpec = (
    Option<usize>,
    Vec<u8>,
    u8,
    CompilerInstructionRelocationRecipe,
);

pub(super) fn expected_compiler_instruction_spec(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    function_instruction_count: usize,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<CompilerInstructionSpec, Diagnostic> {
    if let Some(spec) = control_entry::expected_control_entry_spec(
        architecture,
        code,
        function_instruction_count,
        kind.clone(),
    )? {
        return Ok(spec);
    }

    if let Some(spec) =
        storage_place::expected_storage_place_spec(architecture, code, kind.clone())?
    {
        return Ok(spec);
    }

    if let Some(spec) = outbound_calls::expected_outbound_call_spec(architecture, kind.clone())? {
        return Ok(spec);
    }

    if let Some(spec) =
        buffer_wire_text::expected_buffer_wire_text_spec(architecture, kind.clone())?
    {
        return Ok(spec);
    }
    if let Some(spec) =
        arithmetic_convert::expected_arithmetic_convert_spec(architecture, code, kind)?
    {
        return Ok(spec);
    }

    unreachable!("all compiler instruction specification families were dispatched")
}
