use omega_calling_conventions::HostOperationKey;
use omega_object_file::RelocationKind;
use omega_target::Architecture;
use omega_target_operations::InstructionOperandLike;

pub(crate) fn external_call_relocation_offset<T: InstructionOperandLike>(
    architecture: Architecture,
    operation_key: HostOperationKey,
    selected_text_offset: usize,
    operands: &[T],
) -> usize {
    if architecture == Architecture::X86_64
        && let Some(site) =
            omega_isa_x86_64::host_call_external_relocation_site(operation_key, operands)
    {
        return selected_text_offset + site.byte_offset;
    }

    // AArch64 value-returning layout is `[args (operands[1..])] [BL] [result
    // store]`, so the branch sits after the ARGS only — the result operand[0]
    // is stored after the call, not marshalled before it.
    if architecture == Architecture::Aarch64 && operation_key.returns_value() {
        return selected_text_offset
            + operands
                .iter()
                .skip(1)
                .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
                .sum::<usize>();
    }

    let operand_bytes = operands
        .iter()
        .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
        .sum::<usize>();

    selected_text_offset
        + operand_bytes
        + match architecture {
            Architecture::Aarch64 => 0,
            Architecture::X86_64 => 1,
        }
}

pub(crate) fn external_call_relocation_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 4,
    }
}

pub(crate) fn external_call_relocation_kind(architecture: Architecture) -> RelocationKind {
    match architecture {
        Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
        Architecture::X86_64 => RelocationKind::X86_64Relative32,
    }
}
