use omega_calling_conventions::HostOperationKey;
use omega_object_file::RelocationKind;
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::InstructionOperandLike;

#[cfg(test)]
pub(crate) fn external_call_relocation_offset<T: InstructionOperandLike>(
    target: NativeTarget,
    operation_key: HostOperationKey,
    selected_text_offset: usize,
    operands: &[T],
    authored_import: bool,
) -> usize {
    external_call_relocation_offset_with_plan(
        target,
        operation_key,
        selected_text_offset,
        operands,
        authored_import,
        None,
    )
}

pub(crate) fn external_call_relocation_offset_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operation_key: HostOperationKey,
    selected_text_offset: usize,
    operands: &[T],
    authored_import: bool,
    authoritative_plan: Option<&omega_calling_conventions::CallPlan>,
) -> usize {
    let architecture = target.architecture;
    if architecture == Architecture::X86_64
        && let Some(plan) = authoritative_plan
        && let Some(site) = omega_isa_x86_64::authored_import_relocation_sites(plan, operands)
            .into_iter()
            .find(|site| site.kind == omega_isa_x86_64::X86_64RelocationSiteKind::Relative32)
    {
        return selected_text_offset + site.byte_offset;
    }
    if architecture == Architecture::X86_64
        && let Some(site) = omega_isa_x86_64::host_call_external_relocation_site_for_policy(
            omega_calling_conventions::CallingPolicy::native_for_target(target),
            operation_key,
            operands,
        )
    {
        return selected_text_offset + site.byte_offset;
    }

    // AArch64 value-returning layout is `[args (operands[1..])] [BL] [result
    // store]`, so the branch sits after the ARGS only — the result operand[0]
    // is stored after the call, not marshalled before it. A stack-mode op
    // (`open_create`) inserts `sub sp` + `str [sp]` (8 bytes) between the register
    // args and the `BL` (the `add sp` is AFTER the BL, so it does not shift it),
    // beyond counting the mode immediate as a register arg.
    // An AUTHORED import (provides row / via leaf, custom capability) always
    // rides the value-returning layout -- the blocker enforces the
    // result-binding shape and the encoder routes it there; the catalog
    // cannot know authored operations.
    if architecture == Architecture::Aarch64 && (operation_key.returns_value() || authored_import) {
        let argument_placements =
            omega_instruction_selection::normalized_aarch64_host_argument_placements(
                operation_key,
                operands,
                authored_import,
            )
            .unwrap_or_default();
        let stack_mode_bytes = if operation_key.passes_trailing_mode_on_stack() {
            8
        } else {
            0
        };
        return selected_text_offset
            + operands
                .iter()
                .skip(1)
                .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
                .sum::<usize>()
            + stack_mode_bytes
            + omega_instruction_selection::aarch64_host_call_stack_prefix_width_for_placements(
                &argument_placements,
                argument_placements.len(),
            );
    }

    let operand_bytes = operands
        .iter()
        .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
        .sum::<usize>();

    let planned_stack_bytes = if architecture == Architecture::Aarch64 {
        omega_instruction_selection::normalized_aarch64_host_argument_placements(
            operation_key,
            operands,
            false,
        )
        .map(|placements| {
            omega_instruction_selection::aarch64_host_call_stack_prefix_width_for_placements(
                &placements,
                placements.len(),
            )
        })
        .unwrap_or(0)
    } else {
        0
    };

    selected_text_offset
        + operand_bytes
        + planned_stack_bytes
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

#[cfg(test)]
mod tests {
    use super::external_call_relocation_offset;
    use omega_assigned_target_operations::{InstructionOperand, InstructionOperandKind};
    use omega_target::NativeTarget;
    use omega_target_operations::RuntimeStorageRegion;

    #[test]
    fn authored_sysv_call_relocation_follows_aggregate_marshalling() {
        let operands = [
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeSmallAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 0,
                    byte_count: 16,
                    alignment: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 32,
                    byte_count: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeSmallAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 40,
                    byte_count: 16,
                    alignment: 8,
                },
            },
        ];

        assert_eq!(
            external_call_relocation_offset(
                NativeTarget::linux_x64(),
                omega_calling_conventions::HostOperationKey::default(),
                20,
                &operands,
                true,
            ),
            66
        );
    }
}
