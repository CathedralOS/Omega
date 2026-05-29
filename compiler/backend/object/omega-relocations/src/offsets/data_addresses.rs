use omega_assigned_target_operations::InstructionOperand;
use omega_calling_conventions::HostOperationKey;
use omega_target::Architecture;

pub(crate) fn data_address_relocation_offset(
    architecture: Architecture,
    operation_key: Option<HostOperationKey>,
    operands: &[InstructionOperand],
    selected_text_offset: usize,
    operand_index: usize,
) -> usize {
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
            data_address_relocation_offset(Architecture::Aarch64, None, &operands, 20, 1),
            24
        );
        assert_eq!(
            data_address_relocation_offset(Architecture::X86_64, None, &operands, 20, 1),
            28
        );
    }
}
