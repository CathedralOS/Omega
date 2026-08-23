use omega_assigned_target_operations::InstructionOperand;
use omega_calling_conventions::HostOperationKey;
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::InstructionOperandLike;

use super::CallPlanSource;

/// The fixup-relevant shape of a field-model (vtable/table-function) call:
/// whether the receiver is a wire argument and whether a result place leads
/// the operands. Computed from the binding mechanism at collection.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldModelCallShape {
    pub passes_receiver: bool,
    pub result_present: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn data_address_relocation_offset_for_target_with_plan(
    target: NativeTarget,
    operation_key: Option<HostOperationKey>,
    operands: &[InstructionOperand],
    selected_text_offset: usize,
    operand_index: usize,
    is_syscall: bool,
    field_model_shape: Option<FieldModelCallShape>,
    authored_import: bool,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    data_address_relocation_offset_for_target_for_plan(
        target,
        operation_key,
        operands,
        selected_text_offset,
        operand_index,
        is_syscall,
        field_model_shape,
        authored_import,
        CallPlanSource::Authoritative(authoritative_plan),
    )
}

/// Return the one data relocation owned by a selected constant-result row.
///
/// Constant-result lowering has no boundary call and therefore no `CallPlan`.
/// Keep that fixed instruction geometry separate from the compatibility ABI
/// oracle so production relocation cannot mistake absent boundary evidence for
/// permission to reconstruct a call layout.
pub(crate) fn constant_result_data_relocation_offset_for_target(
    target: NativeTarget,
    selected_text_offset: usize,
) -> usize {
    selected_text_offset
        + match target.architecture {
            Architecture::Aarch64 => 16,
            Architecture::X86_64 => 12,
        }
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub(crate) fn data_address_relocation_offset_for_target_no_plan(
    target: NativeTarget,
    operation_key: Option<HostOperationKey>,
    operands: &[InstructionOperand],
    selected_text_offset: usize,
    operand_index: usize,
    is_syscall: bool,
    field_model_shape: Option<FieldModelCallShape>,
    authored_import: bool,
) -> usize {
    data_address_relocation_offset_for_target_for_plan(
        target,
        operation_key,
        operands,
        selected_text_offset,
        operand_index,
        is_syscall,
        field_model_shape,
        authored_import,
        CallPlanSource::CompatibilityOracle,
    )
}

#[allow(clippy::too_many_arguments)]
fn data_address_relocation_offset_for_target_for_plan(
    target: NativeTarget,
    operation_key: Option<HostOperationKey>,
    operands: &[InstructionOperand],
    selected_text_offset: usize,
    operand_index: usize,
    is_syscall: bool,
    field_model_shape: Option<FieldModelCallShape>,
    authored_import: bool,
    plan_source: CallPlanSource<'_>,
) -> usize {
    let authoritative_plan = plan_source.authoritative();
    if operand_index == 0
        && target.object_format != omega_target::ObjectFormat::Coff
        && operation_key.is_some_and(HostOperationKey::lowers_to_constant_result)
    {
        return selected_text_offset
            + match target.architecture {
                Architecture::Aarch64 => 16,
                Architecture::X86_64 => 12,
            };
    }
    if target.architecture == Architecture::X86_64
        && field_model_shape.is_none()
        && authored_import
        && let Some(plan) = authoritative_plan
        && let Some(site) = omega_isa_x86_64::authored_import_relocation_sites(plan, operands)
            .into_iter()
            .find(|site| {
                site.operand_index == Some(operand_index)
                    && site.kind == omega_isa_x86_64::X86_64RelocationSiteKind::Absolute64
            })
    {
        return selected_text_offset + site.byte_offset;
    }
    if target.architecture == Architecture::X86_64
        && let Some(plan) = authoritative_plan
        && plan.policy == omega_calling_conventions::CallingPolicy::SystemVAMD64
        && let Some(shape) = field_model_shape
    {
        let byte_offset = if shape.passes_receiver {
            omega_isa_x86_64::sysv_vtable_call_data_relocation_byte_offset_with_plan(
                operands,
                0,
                shape.result_present,
                operand_index,
                plan,
            )
        } else {
            omega_isa_x86_64::sysv_table_function_call_data_relocation_byte_offset_with_plan(
                operands,
                0,
                shape.result_present,
                operand_index,
                plan,
            )
        };
        return selected_text_offset + byte_offset;
    }
    if target.architecture == Architecture::X86_64
        && field_model_shape.is_none()
        && !is_syscall
        && let Some(operation_key) = operation_key
        && let Some(site) = match plan_source {
            CallPlanSource::Authoritative(plan) => {
                omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                    omega_calling_conventions::CallingPolicy::native_for_target(target),
                    operation_key,
                    operands,
                    operand_index,
                    plan,
                )
            }
            CallPlanSource::CompatibilityOracle => {
                omega_isa_x86_64::host_call_data_relocation_site_no_plan(
                    omega_calling_conventions::CallingPolicy::native_for_target(target),
                    operation_key,
                    operands,
                    operand_index,
                )
            }
        }
    {
        return selected_text_offset + site.byte_offset;
    }
    data_address_relocation_offset_for_plan(
        target.architecture,
        operation_key,
        operands,
        selected_text_offset,
        operand_index,
        is_syscall,
        field_model_shape,
        authored_import,
        plan_source,
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
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
    data_address_relocation_offset_for_plan(
        architecture,
        operation_key,
        operands,
        selected_text_offset,
        operand_index,
        is_syscall,
        field_model_shape,
        authored_import,
        CallPlanSource::CompatibilityOracle,
    )
}

#[allow(clippy::too_many_arguments)]
fn data_address_relocation_offset_for_plan(
    architecture: Architecture,
    operation_key: Option<HostOperationKey>,
    operands: &[InstructionOperand],
    selected_text_offset: usize,
    operand_index: usize,
    is_syscall: bool,
    field_model_shape: Option<FieldModelCallShape>,
    authored_import: bool,
    plan_source: CallPlanSource<'_>,
) -> usize {
    let authoritative_plan = plan_source.authoritative();
    let discards_native_result = operation_key
        .is_some_and(omega_calling_conventions::HostOperationKey::discards_native_result);
    let plan_returns_value =
        authoritative_plan.map(|plan| !discards_native_result && plan.result.is_some());
    let plan_returns_float = authoritative_plan
        .and_then(|plan| plan.result.as_ref())
        .is_some_and(|result| {
            !discards_native_result && {
                matches!(
                    result.shape.class,
                    omega_calling_conventions::ValueClass::Float
                )
            }
        });
    if is_syscall
        && operand_index == 0
        && operation_key.is_some_and(HostOperationKey::uses_linux_timespec_result)
    {
        let number = omega_calling_conventions::linux_clock_gettime_syscall_number(architecture);
        let byte_offset = match plan_source {
            CallPlanSource::Authoritative(plan) => {
                omega_instruction_selection::linux_timespec_result_relocation_byte_offset_with_plan(
                    architecture,
                    operands,
                    number,
                    plan,
                )
            }
            CallPlanSource::CompatibilityOracle => {
                omega_instruction_selection::linux_timespec_result_relocation_byte_offset_no_plan(
                    architecture,
                    operands,
                    number,
                )
            }
        };
        if let Ok(byte_offset) = byte_offset {
            return selected_text_offset + byte_offset;
        }
    }
    if is_syscall
        && operand_index == 0
        && operation_key.is_some_and(HostOperationKey::uses_linux_timespec_argument)
    {
        let number = omega_calling_conventions::linux_nanosleep_syscall_number(architecture);
        let byte_offset = match plan_source {
            CallPlanSource::Authoritative(plan) =>
                omega_instruction_selection::linux_timespec_argument_relocation_byte_offset_with_plan(
                    architecture, operands, number, plan,
                ),
            CallPlanSource::CompatibilityOracle =>
                omega_instruction_selection::linux_timespec_argument_relocation_byte_offset_no_plan(
                    architecture, operands, number,
                ),
        };
        if let Ok(Some(byte_offset)) = byte_offset {
            return selected_text_offset + byte_offset;
        }
    }
    if is_syscall
        && plan_returns_value.unwrap_or_else(|| {
            operation_key.is_some_and(HostOperationKey::returns_value)
        })
        // Linux syscall numbers fit one normalized immediate chunk on AArch64;
        // zero therefore has the same instruction width while avoiding a
        // second syscall-number table in the relocation planner.
        && let Ok(byte_offset) = match plan_source {
            CallPlanSource::Authoritative(plan) =>
                omega_instruction_selection::value_syscall_relocation_byte_offset_with_plan(
                    architecture,
                    operands,
                    operand_index,
                    0,
                    plan,
                ),
            CallPlanSource::CompatibilityOracle => omega_instruction_selection::value_syscall_relocation_byte_offset_no_plan(
                architecture,
                operands,
                operand_index,
                0,
            ),
        }
    {
        return selected_text_offset + byte_offset;
    }

    // A field-model call marshals args like an import, then reads the callee
    // from the receiver (This-call) or from the dispatch-only table pointer
    // (service table) -- each shape has its own fixup layout.
    if architecture == Architecture::X86_64
        && let Some(shape) = field_model_shape
        && let Some(plan) = authoritative_plan
        && plan.policy == omega_calling_conventions::CallingPolicy::MicrosoftX64
    {
        let byte_offset = if shape.passes_receiver {
            omega_isa_x86_64::win64_vtable_call_relocation_sites_with_plan(
                operands,
                shape.result_present,
                plan,
            )
            .into_iter()
            .find(|site| site.operand_index == Some(operand_index))
            .map(|site| site.byte_offset)
            .unwrap_or(0)
        } else {
            omega_isa_x86_64::win64_table_function_call_relocation_sites_with_plan(
                operands,
                shape.result_present,
                plan,
            )
            .into_iter()
            .find(|site| site.operand_index == Some(operand_index))
            .map(|site| site.byte_offset)
            .unwrap_or(0)
        };
        return selected_text_offset + byte_offset;
    }

    // AArch64 This-call field dispatch uses the direct-import marshaller, but
    // a leading result place is stored only after the arguments, indirect
    // load/BLR, and outgoing-stack restore. Keep every page relocation on the
    // exact byte selected by that layout.
    if architecture == Architecture::Aarch64
        && let Some(shape) = field_model_shape
        && shape.passes_receiver
        && let Some(plan) = authoritative_plan
        && let Ok((argument_placements, _)) =
            omega_instruction_selection::normalized_aarch64_vtable_plan_with_plan(
                operands,
                shape.result_present,
                plan,
            )
    {
        let argument_start = usize::from(shape.result_present);
        if shape.result_present && operand_index == 0 {
            if operands
                .first()
                .and_then(omega_target_operations::InstructionOperandLike::runtime_large_aggregate)
                .is_some()
            {
                return selected_text_offset;
            }
            let float_result_move = usize::from(
                operands
                    .first()
                    .is_some_and(|operand| operand.runtime_scalar_float().is_some()),
            ) * 4;
            return selected_text_offset
                + operands[argument_start..]
                    .iter()
                    .map(|operand| {
                        omega_instruction_selection::operand_width(architecture, operand)
                    })
                    .sum::<usize>()
                + omega_instruction_selection::aarch64_host_call_stack_total_width_for_placements(
                    &argument_placements,
                )
                + 8
                + float_result_move;
        }
        if operand_index >= argument_start {
            let result_prefix = if shape.result_present {
                operands
                    .first()
                    .and_then(omega_instruction_selection::aarch64_indirect_result_address_width)
                    .unwrap_or(0)
            } else {
                0
            };
            return selected_text_offset
                + result_prefix
                + operands[argument_start..operand_index]
                    .iter()
                    .map(|operand| {
                        omega_instruction_selection::operand_width(architecture, operand)
                    })
                    .sum::<usize>()
                + omega_instruction_selection::aarch64_host_call_stack_prefix_width_for_placements(
                    &argument_placements,
                    operand_index - argument_start,
                );
        }
    }

    // AArch64 dispatch-only service tables marshal only the declared
    // arguments first, then materialize the table storage operand and read the
    // function field. The table itself never consumes an AAPCS64 parameter.
    if architecture == Architecture::Aarch64
        && let Some(shape) = field_model_shape
        && !shape.passes_receiver
        && let Some(plan) = authoritative_plan
        && let Ok((argument_placements, _)) =
            omega_instruction_selection::normalized_aarch64_table_function_plan_with_plan(
                operands,
                shape.result_present,
                plan,
            )
    {
        let table_index = usize::from(shape.result_present);
        let argument_start = table_index + 1;
        let argument_width = |end: usize| {
            operands[argument_start..end]
                .iter()
                .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
                .sum::<usize>()
        };
        if shape.result_present && operand_index == 0 {
            if operands
                .first()
                .and_then(omega_target_operations::InstructionOperandLike::runtime_large_aggregate)
                .is_some()
            {
                return selected_text_offset;
            }
            let float_result_move = usize::from(
                operands
                    .first()
                    .is_some_and(|operand| operand.runtime_scalar_float().is_some()),
            ) * 4;
            return selected_text_offset
                + argument_width(operands.len())
                + omega_instruction_selection::operand_width(architecture, &operands[table_index])
                + omega_instruction_selection::aarch64_host_call_stack_total_width_for_placements(
                    &argument_placements,
                )
                + 8
                + float_result_move;
        }
        if operand_index == table_index {
            let result_prefix = if shape.result_present {
                operands
                    .first()
                    .and_then(omega_instruction_selection::aarch64_indirect_result_address_width)
                    .unwrap_or(0)
            } else {
                0
            };
            return selected_text_offset
                + result_prefix
                + argument_width(operands.len())
                + omega_instruction_selection::aarch64_host_call_stack_prefix_width_for_placements(
                    &argument_placements,
                    argument_placements.len(),
                );
        }
        if operand_index >= argument_start {
            let result_prefix = if shape.result_present {
                operands
                    .first()
                    .and_then(omega_instruction_selection::aarch64_indirect_result_address_width)
                    .unwrap_or(0)
            } else {
                0
            };
            return selected_text_offset
                + result_prefix
                + argument_width(operand_index)
                + omega_instruction_selection::aarch64_host_call_stack_prefix_width_for_placements(
                    &argument_placements,
                    operand_index - argument_start,
                );
        }
    }
    // x86_64 Linux syscalls marshal each argument into its register independently, so
    // the data-address/runtime-storage fixup is the sum of the preceding arguments'
    // marshalling widths plus 2 -- a different layout than the win32 import sequence.
    if architecture == Architecture::X86_64 && is_syscall {
        return selected_text_offset
            + omega_isa_x86_64::syscall_data_relocation_byte_offset(operands, operand_index);
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
        && (plan_returns_value.unwrap_or_else(|| operation_key.returns_value())
            // Authored imports (custom capability) always ride the
            // value-returning layout; the flag arrives from the record
            // walker, which sees the binding mechanism.
            || authored_import)
    {
        let argument_placements = match plan_source {
            CallPlanSource::Authoritative(plan) => {
                omega_instruction_selection::normalized_aarch64_host_argument_placements_with_plan(
                    operation_key,
                    operands,
                    authored_import,
                    plan,
                )
            }
            CallPlanSource::CompatibilityOracle => {
                omega_instruction_selection::normalized_aarch64_host_argument_placements_no_plan(
                    operation_key,
                    operands,
                    authored_import,
                )
            }
        }
        .unwrap_or_default();
        let arg_bytes = |range: std::ops::Range<usize>| {
            operands[range]
                .iter()
                .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
                .sum::<usize>()
        };
        if operand_index == 0 {
            if operands
                .first()
                .and_then(omega_target_operations::InstructionOperandLike::runtime_large_aggregate)
                .is_some()
            {
                return selected_text_offset;
            }
            // The result store's adrp/add lands after the args + the BL (4). A
            // deref-result op (errno) inserts an extra `ldr w0,[x0]` (4) between
            // the BL and the store, pushing the store's page-pair 4 bytes later.
            // A float-returning op (sqrt/hypot) inserts an extra `fmov x0,d0` (4)
            // in that same slot (same shift). Outgoing stack setup and teardown
            // come from the same normalized placements consumed by the encoder.
            let deref_bytes = if operation_key.dereferences_result() {
                4
            } else {
                0
            };
            let float_return_bytes = if plan_returns_float
                || (matches!(plan_source, CallPlanSource::CompatibilityOracle)
                    && operation_key.returns_float())
                || (matches!(plan_source, CallPlanSource::CompatibilityOracle)
                    && authored_import
                    && operands
                        .first()
                        .is_some_and(|operand| operand.runtime_scalar_float().is_some()))
            {
                4
            } else {
                0
            };
            return selected_text_offset
                + arg_bytes(1..operands.len())
                + 4
                + deref_bytes
                + float_return_bytes
                + omega_instruction_selection::aarch64_host_call_stack_total_width_for_placements(
                    &argument_placements,
                );
        }
        let result_prefix = operands
            .first()
            .and_then(omega_instruction_selection::aarch64_indirect_result_address_width)
            .unwrap_or(0);
        return selected_text_offset
            + result_prefix
            + arg_bytes(1..operand_index)
            + omega_instruction_selection::aarch64_host_call_stack_prefix_width_for_placements(
                &argument_placements,
                operand_index - 1,
            );
    }

    if architecture == Architecture::Aarch64
        && let Some(operation_key) = operation_key
        && let Ok(argument_placements) = match plan_source {
            CallPlanSource::Authoritative(plan) => {
                omega_instruction_selection::normalized_aarch64_host_argument_placements_with_plan(
                    operation_key,
                    operands,
                    false,
                    plan,
                )
            }
            CallPlanSource::CompatibilityOracle => {
                omega_instruction_selection::normalized_aarch64_host_argument_placements_no_plan(
                    operation_key,
                    operands,
                    false,
                )
            }
        }
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
    use super::{
        FieldModelCallShape, constant_result_data_relocation_offset_for_target,
        data_address_relocation_offset, data_address_relocation_offset_for_target_no_plan,
        data_address_relocation_offset_for_target_with_plan,
    };
    use omega_assigned_target_operations::{InstructionOperand, InstructionOperandKind};
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, HostCapability, HostOperation, HostOperationKey,
        ValueLocation, ValueShape, evaluate_call_plan,
    };
    use omega_target::{Architecture, NativeTarget};
    use omega_target_operations::{RuntimeStorageRegion, TargetDataObject};
    use psi_arena::Handle;

    fn darwin_open_create_operands() -> [InstructionOperand; 4] {
        [
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
        ]
    }

    #[test]
    fn constant_result_relocation_uses_its_non_boundary_geometry() {
        assert_eq!(
            constant_result_data_relocation_offset_for_target(NativeTarget::linux_arm64(), 20,),
            36
        );
        assert_eq!(
            constant_result_data_relocation_offset_for_target(NativeTarget::windows_x64(), 20,),
            32
        );
    }

    #[test]
    fn darwin_open_create_data_relocations_follow_the_complete_variadic_plan() {
        let operands = darwin_open_create_operands();
        let operation = Some(HostOperationKey::new(
            HostCapability::Filesystem,
            HostOperation::OpenCreate,
        ));

        assert_eq!(
            data_address_relocation_offset_for_target_no_plan(
                NativeTarget::macos_arm64(),
                operation,
                &operands,
                20,
                1,
                false,
                None,
                false,
            ),
            24
        );
        assert_eq!(
            data_address_relocation_offset_for_target_no_plan(
                NativeTarget::macos_arm64(),
                operation,
                &operands,
                20,
                0,
                false,
                None,
                false,
            ),
            52
        );
    }

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

    #[test]
    fn authored_aarch64_float_result_relocation_follows_the_vector_move() {
        let operands = [
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarFloat {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 32,
                    byte_count: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(7),
            },
        ];

        assert_eq!(
            data_address_relocation_offset(
                Architecture::Aarch64,
                Some(omega_calling_conventions::HostOperationKey::default()),
                &operands,
                20,
                0,
                false,
                None,
                true,
            ),
            32
        );
    }

    #[test]
    fn authored_aarch64_data_relocation_uses_the_source_selected_stack_plan() {
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
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_arm64(),
                Some(omega_calling_conventions::HostOperationKey::default()),
                &operands,
                20,
                1,
                false,
                None,
                true,
                &plan,
            ),
            24
        );
    }

    #[test]
    fn void_aarch64_data_relocation_uses_the_retained_stack_plan() {
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
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_arm64(),
                Some(omega_calling_conventions::HostOperationKey::default()),
                &operands,
                20,
                0,
                false,
                None,
                false,
                &plan,
            ),
            24
        );
    }

    #[test]
    fn authored_aarch64_small_aggregate_result_relocation_follows_the_call() {
        let operands = [
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeSmallAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 64,
                    byte_count: 16,
                    alignment: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(7),
            },
        ];

        assert_eq!(
            data_address_relocation_offset(
                Architecture::Aarch64,
                Some(omega_calling_conventions::HostOperationKey::default()),
                &operands,
                20,
                0,
                false,
                None,
                true,
            ),
            28
        );
    }

    #[test]
    fn authored_sysv_aggregate_relocations_follow_the_plan_driven_layout() {
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
        let operation = Some(omega_calling_conventions::HostOperationKey::default());

        assert_eq!(
            data_address_relocation_offset_for_target_no_plan(
                NativeTarget::linux_x64(),
                operation,
                &operands,
                20,
                1,
                false,
                None,
                true,
            ),
            26
        );
        assert_eq!(
            data_address_relocation_offset_for_target_no_plan(
                NativeTarget::linux_x64(),
                operation,
                &operands,
                20,
                2,
                false,
                None,
                true,
            ),
            43
        );
        assert_eq!(
            data_address_relocation_offset_for_target_no_plan(
                NativeTarget::linux_x64(),
                operation,
                &operands,
                20,
                0,
                false,
                None,
                true,
            ),
            76
        );
    }

    #[test]
    fn sysv_indirect_field_relocations_follow_the_normalized_layouts() {
        let scalar = |byte_offset| InstructionOperand {
            kind: InstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 8,
            },
        };
        let operands = [scalar(0), scalar(8), scalar(16)];
        let operation = Some(omega_calling_conventions::HostOperationKey::default());

        let vtable = Some(FieldModelCallShape {
            passes_receiver: true,
            result_present: true,
        });
        let vtable_signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 2],
            result: Some(ValueShape::integer(8, 8)),
        };
        let source_plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &vtable_signature)
            .expect("source-selected SysV vtable plan");
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_x64(),
                operation,
                &operands,
                20,
                1,
                false,
                vtable,
                false,
                &source_plan,
            ),
            26
        );
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_x64(),
                operation,
                &operands,
                20,
                0,
                false,
                vtable,
                false,
                &source_plan,
            ),
            73
        );
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::windows_x64(),
                operation,
                &operands,
                20,
                1,
                false,
                vtable,
                false,
                &source_plan,
            ),
            26
        );
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::windows_x64(),
                operation,
                &operands,
                20,
                0,
                false,
                vtable,
                false,
                &source_plan,
            ),
            73
        );

        let table = Some(FieldModelCallShape {
            passes_receiver: false,
            result_present: true,
        });
        let table_signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(8, 8)),
        };
        let table_plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &table_signature)
            .expect("source-selected SysV table plan");
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_x64(),
                operation,
                &operands,
                20,
                2,
                false,
                table,
                false,
                &table_plan,
            ),
            26
        );
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_x64(),
                operation,
                &operands,
                20,
                1,
                false,
                table,
                false,
                &table_plan,
            ),
            43
        );
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_x64(),
                operation,
                &operands,
                20,
                0,
                false,
                table,
                false,
                &table_plan,
            ),
            73
        );
    }

    #[test]
    fn authored_aarch64_indirect_result_precedes_the_caller_copy() {
        let aggregate = || InstructionOperand {
            kind: InstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 24,
                alignment: 8,
            },
        };
        let operands = [aggregate(), aggregate()];

        assert_eq!(
            data_address_relocation_offset(
                Architecture::Aarch64,
                Some(omega_calling_conventions::HostOperationKey::default()),
                &operands,
                20,
                0,
                false,
                None,
                true,
            ),
            20
        );
        assert_eq!(
            data_address_relocation_offset(
                Architecture::Aarch64,
                Some(omega_calling_conventions::HostOperationKey::default()),
                &operands,
                20,
                1,
                false,
                None,
                true,
            ),
            36
        );
    }

    #[test]
    fn aarch64_vtable_result_relocation_follows_arguments_and_indirect_dispatch() {
        let operands = [
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 32,
                    byte_count: 4,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 0,
                    byte_count: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(7),
            },
        ];
        let shape = Some(FieldModelCallShape {
            passes_receiver: true,
            result_present: true,
        });
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 2],
            result: Some(ValueShape::integer(4, 4)),
        };
        let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
            .expect("retained AAPCS64 vtable plan");

        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_arm64(),
                None,
                &operands,
                20,
                0,
                false,
                shape,
                false,
                &plan,
            ),
            44
        );
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_arm64(),
                None,
                &operands,
                20,
                1,
                false,
                shape,
                false,
                &plan,
            ),
            20
        );

        let data_operands = [
            operands[0].clone(),
            operands[1].clone(),
            InstructionOperand {
                kind: InstructionOperandKind::DataAddress {
                    data: Handle::<TargetDataObject>::invalid(),
                },
            },
        ];
        let mut source_plan = plan;
        source_plan.parameters[1].locations[0] = ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        };
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_arm64(),
                None,
                &data_operands,
                20,
                2,
                false,
                shape,
                false,
                &source_plan,
            ),
            36
        );

        let float_operands = [
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarFloat {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 32,
                    byte_count: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 0,
                    byte_count: 8,
                },
            },
        ];
        let float_signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::float(8)),
        };
        let float_plan = evaluate_call_plan(CallingPolicy::Aapcs64, &float_signature)
            .expect("retained AAPCS64 float vtable plan");
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_arm64(),
                None,
                &float_operands,
                20,
                0,
                false,
                shape,
                false,
                &float_plan,
            ),
            44
        );
    }

    #[test]
    fn aarch64_field_indirect_result_relocation_precedes_dispatch_inputs() {
        let result = || InstructionOperand {
            kind: InstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 24,
                alignment: 8,
            },
        };
        let pointer = || InstructionOperand {
            kind: InstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            },
        };

        for (passes_receiver, operands) in [
            (true, [result(), pointer()]),
            (false, [result(), pointer()]),
        ] {
            let shape = Some(FieldModelCallShape {
                passes_receiver,
                result_present: true,
            });
            let signature = CallSignature {
                parameters: if passes_receiver {
                    vec![ValueShape::integer(8, 8)]
                } else {
                    Vec::new()
                },
                result: Some(ValueShape::integer(24, 8)),
            };
            let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
                .expect("retained AAPCS64 indirect field plan");
            assert_eq!(
                data_address_relocation_offset_for_target_with_plan(
                    NativeTarget::linux_arm64(),
                    None,
                    &operands,
                    20,
                    0,
                    false,
                    shape,
                    false,
                    &plan,
                ),
                20
            );
            assert_eq!(
                data_address_relocation_offset_for_target_with_plan(
                    NativeTarget::linux_arm64(),
                    None,
                    &operands,
                    20,
                    1,
                    false,
                    shape,
                    false,
                    &plan,
                ),
                32
            );
        }
    }

    #[test]
    fn aarch64_table_pointer_relocation_follows_only_wire_arguments() {
        let operands = [
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 32,
                    byte_count: 4,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 40,
                    byte_count: 8,
                },
            },
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(7),
            },
        ];
        let shape = Some(FieldModelCallShape {
            passes_receiver: false,
            result_present: true,
        });
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(4, 4)),
        };
        let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
            .expect("retained AAPCS64 table plan");

        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_arm64(),
                None,
                &operands,
                20,
                1,
                false,
                shape,
                false,
                &plan,
            ),
            24
        );
        assert_eq!(
            data_address_relocation_offset_for_target_with_plan(
                NativeTarget::linux_arm64(),
                None,
                &operands,
                20,
                0,
                false,
                shape,
                false,
                &plan,
            ),
            44
        );
    }
}
