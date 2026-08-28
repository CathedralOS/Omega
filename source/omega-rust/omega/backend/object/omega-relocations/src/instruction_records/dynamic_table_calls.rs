use super::super::data_addresses::collect_data_address_relocations;
use super::super::offsets::FieldModelCallShape;
use super::context::InstructionRelocationContext;
use omega_target_operations::{InstructionOperandLike, SelectedInstructionKind};
use psi_diagnostics::Diagnostic;

pub(super) fn collect_dynamic_table_call_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> Result<bool, Diagnostic> {
    let SelectedInstructionKind::DynamicTableCall {
        result_present,
        call_plan,
        operands,
        ..
    } = instruction
    else {
        return Ok(false);
    };
    let assigned = context
        .input
        .assigned_target_operations
        .instruction_operands(*operands)
        .ok_or_else(|| {
            Diagnostic::error("private dynamic table call lost its assigned operands")
        })?;
    let table_index = usize::from(*result_present);
    if assigned.len()
        != call_plan
            .parameters
            .len()
            .saturating_add(table_index)
            .saturating_add(1)
        || assigned
            .get(table_index)
            .and_then(InstructionOperandLike::runtime_scalar_integer)
            .is_none()
        || assigned
            .iter()
            .any(|operand| operand.data_address().is_some())
    {
        return Err(Diagnostic::error(
            "private dynamic table call has a mismatched table/argument relocation shape",
        ));
    }

    collect_data_address_relocations(
        context.input,
        context.function_symbol_handle,
        context.selected_instruction_index,
        None,
        *operands,
        context.selected_text_offset,
        Some((
            call_plan,
            FieldModelCallShape {
                passes_receiver: false,
                result_present: *result_present,
            },
        )),
        context.relocation_plan,
    );
    Ok(true)
}
