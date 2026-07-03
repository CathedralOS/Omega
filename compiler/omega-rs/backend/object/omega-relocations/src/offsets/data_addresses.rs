use omega_assigned_target_operations::InstructionOperand;
use omega_calling_conventions::HostOperationKey;
use omega_target::Architecture;

#[allow(clippy::too_many_arguments)]
pub(crate) fn data_address_relocation_offset(
    architecture: Architecture,
    operation_key: Option<HostOperationKey>,
    operands: &[InstructionOperand],
    selected_text_offset: usize,
    operand_index: usize,
    is_syscall: bool,
    is_vtable: bool,
) -> usize {
    // A VtableSlot call marshals args like an import (no leading result), then
    // reads the callee from RCX -- the arg region-base fixups follow the
    // vtable-call layout.
    if architecture == Architecture::X86_64 && is_vtable {
        return selected_text_offset
            + omega_isa_x86_64::vtable_call_data_relocation_byte_offset(operands, operand_index);
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
            data_address_relocation_offset(Architecture::Aarch64, None, &operands, 20, 1, false, false),
            24
        );
        assert_eq!(
            data_address_relocation_offset(Architecture::X86_64, None, &operands, 20, 1, false, false),
            28
        );
        // x86_64 Linux syscall layout: arg 1's data-address fixup is at 20 + 1*10 + 2.
        assert_eq!(
            data_address_relocation_offset(Architecture::X86_64, None, &operands, 20, 1, true, false),
            32
        );
    }
}
