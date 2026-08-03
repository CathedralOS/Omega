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
    let returns_value = authoritative_plan
        .map(|plan| plan.result.is_some())
        .unwrap_or_else(|| operation_key.returns_value());
    if architecture == Architecture::X86_64
        && let Some(site) = authoritative_plan
            .and_then(|plan| {
                omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                    omega_calling_conventions::CallingPolicy::native_for_target(target),
                    operation_key,
                    operands,
                    plan,
                )
            })
            .or_else(|| {
                omega_isa_x86_64::host_call_external_relocation_site_for_policy(
                    omega_calling_conventions::CallingPolicy::native_for_target(target),
                    operation_key,
                    operands,
                )
            })
    {
        return selected_text_offset + site.byte_offset;
    }

    // AArch64 value-returning layout is `[args (operands[1..])] [BL] [result
    // store]`, so the branch sits after the ARGS only — the result operand[0]
    // is stored after the call, not marshalled before it. Outgoing stack setup
    // comes from the same normalized placements consumed by the encoder.
    // A source-authored external import (custom capability) always
    // rides the value-returning layout -- the blocker enforces the
    // result-binding shape and the encoder routes it there; the catalog
    // cannot know authored operations.
    if architecture == Architecture::Aarch64 && (returns_value || authored_import) {
        let argument_placements = match authoritative_plan {
            Some(plan) => {
                omega_instruction_selection::normalized_aarch64_host_argument_placements_with_plan(
                    operation_key,
                    operands,
                    authored_import,
                    plan,
                )
            }
            None => omega_instruction_selection::normalized_aarch64_host_argument_placements(
                operation_key,
                operands,
                authored_import,
            ),
        }
        .unwrap_or_default();
        return selected_text_offset
            + operands
                .iter()
                .skip(1)
                .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
                .sum::<usize>()
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
        match authoritative_plan {
            Some(plan) => {
                omega_instruction_selection::normalized_aarch64_host_argument_placements_with_plan(
                    operation_key,
                    operands,
                    false,
                    plan,
                )
            }
            None => omega_instruction_selection::normalized_aarch64_host_argument_placements(
                operation_key,
                operands,
                false,
            ),
        }
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
    use super::{external_call_relocation_offset, external_call_relocation_offset_with_plan};
    use omega_assigned_target_operations::{InstructionOperand, InstructionOperandKind};
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, HostCapability, HostOperation, HostOperationKey,
        ValueLocation, ValueShape, evaluate_call_plan,
    };
    use omega_target::NativeTarget;
    use omega_target_operations::{RuntimeStorageRegion, TargetDataObject};
    use psi_arena::Handle;

    #[test]
    fn darwin_open_create_branch_relocation_follows_the_complete_variadic_plan() {
        let operands = [
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 0,
                    byte_count: 4,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::DataAddress {
                    data: Handle::<TargetDataObject>::invalid(),
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(0x201),
            },
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(0o644),
            },
        ];

        assert_eq!(
            external_call_relocation_offset(
                NativeTarget::macos_arm64(),
                HostOperationKey::new(HostCapability::Filesystem, HostOperation::OpenCreate),
                20,
                &operands,
                false,
            ),
            44
        );
    }

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

    #[test]
    fn authored_aarch64_call_relocation_uses_the_source_selected_stack_plan() {
        let operands = [
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 0,
                    byte_count: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::DataAddress {
                    data: Handle::<TargetDataObject>::invalid(),
                },
            },
        ];
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(8, 8)),
        };
        let mut plan =
            evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("baseline AAPCS64 plan");
        plan.parameters[0].locations[0] = ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        };

        assert_eq!(
            external_call_relocation_offset_with_plan(
                NativeTarget::linux_arm64(),
                omega_calling_conventions::HostOperationKey::default(),
                20,
                &operands,
                true,
                Some(&plan),
            ),
            36
        );
    }

    #[test]
    fn void_aarch64_call_relocation_uses_the_retained_stack_plan() {
        let operands = [InstructionOperand {
            kind: InstructionOperandKind::DataAddress {
                data: Handle::<TargetDataObject>::invalid(),
            },
        }];
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let mut plan =
            evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("baseline AAPCS64 plan");
        plan.parameters[0].locations[0] = ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        };

        assert_eq!(
            external_call_relocation_offset_with_plan(
                NativeTarget::linux_arm64(),
                omega_calling_conventions::HostOperationKey::default(),
                20,
                &operands,
                false,
                Some(&plan),
            ),
            36
        );
    }
}
