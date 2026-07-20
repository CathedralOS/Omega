use omega_assigned_target_operations::InstructionOperand;
use omega_calling_conventions::HostOperationKey;
use omega_target::Architecture;

/// The fixup-relevant shape of a field-model (vtable/table-function) call:
/// whether the receiver is a wire argument and whether a result place leads
/// the operands. Computed from the binding mechanism at collection.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldModelCallShape {
    pub passes_receiver: bool,
    pub result_present: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn data_address_relocation_offset(
    architecture: Architecture,
    operation_key: Option<HostOperationKey>,
    operands: &[InstructionOperand],
    selected_text_offset: usize,
    operand_index: usize,
    is_syscall: bool,
    field_model_shape: Option<FieldModelCallShape>,
    authored_import: bool,
) -> usize {
    // A field-model call marshals args like an import, then reads the callee
    // from the receiver (This-call) or from the dispatch-only table pointer
    // (service table) -- each shape has its own fixup layout.
    if architecture == Architecture::X86_64
        && let Some(shape) = field_model_shape
    {
        let byte_offset = if shape.passes_receiver {
            omega_isa_x86_64::vtable_call_data_relocation_byte_offset(
                operands,
                operand_index,
                shape.result_present,
            )
        } else {
            omega_isa_x86_64::table_function_call_data_relocation_byte_offset(
                operands,
                operand_index,
                shape.result_present,
            )
        };
        return selected_text_offset + byte_offset;
    }
    // x86_64 Linux syscalls marshal each argument into its register independently, so
    // the data-address/runtime-storage fixup is the sum of the preceding arguments'
    // marshalling widths plus 2 -- a different layout than the win32 import sequence.
    if architecture == Architecture::X86_64 && is_syscall {
        return selected_text_offset
            + omega_isa_x86_64::syscall_data_relocation_byte_offset(operands, operand_index);
    }
    if architecture == Architecture::X86_64
        && let Some(operation_key) = operation_key
        && let Some(site) =
            omega_isa_x86_64::host_call_data_relocation_site(operation_key, operands, operand_index)
    {
        return selected_text_offset + site.byte_offset;
    }

    // AArch64 CONSTANT-RESULT layout `[imm64 (16, padded)] [adrp/add x16]
    // [store]`: the result operand[0]'s page pair sits at a fixed 16. No
    // other operand relocates (the immediate is inline; there is no call).
    if architecture == Architecture::Aarch64
        && let Some(operation_key) = operation_key
        && operation_key.lowers_to_constant_result()
    {
        return selected_text_offset + 16;
    }

    // AArch64 value-returning layout `[args (operands[1..])] [BL] [result
    // store]`: the result operand[0]'s adrp/add lands AFTER the args + the BL
    // (4 bytes); an arg's adrp/add lands after only the args before it (the
    // result is not marshalled up front).
    if architecture == Architecture::Aarch64
        && let Some(operation_key) = operation_key
        && (operation_key.returns_value()
            // Authored imports (custom capability) always ride the
            // value-returning layout; the flag arrives from the record
            // walker, which sees the binding mechanism.
            || authored_import)
    {
        let argument_placements =
            omega_instruction_selection::normalized_aarch64_host_argument_placements(
                operation_key,
                operands,
                authored_import,
            )
            .unwrap_or_default();
        let arg_bytes = |range: std::ops::Range<usize>| {
            operands[range]
                .iter()
                .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
                .sum::<usize>()
        };
        if operand_index == 0 {
            // The result store's adrp/add lands after the args + the BL (4). A
            // deref-result op (errno) inserts an extra `ldr w0,[x0]` (4) between
            // the BL and the store, pushing the store's page-pair 4 bytes later.
            // A float-returning op (sqrt/hypot) inserts an extra `fmov x0,d0` (4)
            // in that same slot (same shift). A stack-mode op (`open_create`)
            // brackets the call with `sub sp` (before BL) + `str [sp]` (before BL)
            // + `add sp` (after BL) = 12 bytes beyond counting the mode immediate
            // as a register arg.
            let deref_bytes = if operation_key.dereferences_result() {
                4
            } else {
                0
            };
            let float_return_bytes = if operation_key.returns_float() { 4 } else { 0 };
            let stack_mode_bytes = if operation_key.passes_trailing_mode_on_stack() {
                12
            } else {
                0
            };
            return selected_text_offset
                + arg_bytes(1..operands.len())
                + 4
                + deref_bytes
                + float_return_bytes
                + stack_mode_bytes
                + omega_instruction_selection::aarch64_host_call_stack_total_width_for_placements(
                    &argument_placements,
                );
        }
        return selected_text_offset
            + arg_bytes(1..operand_index)
            + omega_instruction_selection::aarch64_host_call_stack_prefix_width_for_placements(
                &argument_placements,
                operand_index - 1,
            );
    }

    if architecture == Architecture::Aarch64
        && let Some(operation_key) = operation_key
        && let Ok(argument_placements) =
            omega_instruction_selection::normalized_aarch64_host_argument_placements(
                operation_key,
                operands,
                false,
            )
    {
        return selected_text_offset
            + operands
                .iter()
                .take(operand_index)
                .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
                .sum::<usize>()
            + omega_instruction_selection::aarch64_host_call_stack_prefix_width_for_placements(
                &argument_placements,
                operand_index,
            );
    }

    selected_text_offset
        + operands
            .iter()
            .take(operand_index)
            .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::data_address_relocation_offset;
    use omega_assigned_target_operations::{InstructionOperand, InstructionOperandKind};
    use omega_target::Architecture;

    #[test]
    fn offsets_data_address_by_prior_operand_widths() {
        let operands = [
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(1),
            },
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(2),
            },
        ];

        assert_eq!(
            data_address_relocation_offset(
                Architecture::Aarch64,
                None,
                &operands,
                20,
                1,
                false,
                None,
                false
            ),
            24
        );
        assert_eq!(
            data_address_relocation_offset(
                Architecture::X86_64,
                None,
                &operands,
                20,
                1,
                false,
                None,
                false
            ),
            28
        );
        // x86_64 Linux syscall layout: arg 1's data-address fixup is at 20 + 1*10 + 2.
        assert_eq!(
            data_address_relocation_offset(
                Architecture::X86_64,
                None,
                &operands,
                20,
                1,
                true,
                None,
                false
            ),
            32
        );
    }
}
