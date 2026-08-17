//! Replays target-specific runtime imports, syscalls, and text boundaries.

use super::*;

mod indirect_calls;
mod runtime_io;
mod syscalls;

pub(super) use runtime_io::{encode_runtime_byte_replay, encode_runtime_line_read_replay};
#[cfg(test)]
pub(super) use syscalls::outbound_syscall_argument_storage_sites;
pub(super) use syscalls::{
    encode_linux_timespec_argument_outbound_syscall, encode_linux_timespec_result_outbound_syscall,
    encode_simple_outbound_syscall, outbound_syscall_argument_data_sites,
    outbound_syscall_data_relocation_targets,
};

pub(super) fn aarch64_outbound_syscall_operand(
    operand: &omega_target_operations::InstructionOperand,
) -> Result<omega_isa_aarch64::Aarch64CallOperand, Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    Ok(if operand.data_address().is_some() {
        omega_isa_aarch64::Aarch64CallOperand::DataAddress
    } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeStringPointer {
            byte_offset,
            is_bounded_buffer: operand.runtime_string_is_bounded_buffer(),
        }
    } else if let Some((_, byte_offset)) = operand.runtime_string_length() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeStringLength {
            byte_offset,
            is_bounded_buffer: operand.runtime_string_is_bounded_buffer(),
        }
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_pointer() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimePointeeStringPointer { byte_offset }
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_length() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimePointeeStringLength { byte_offset }
    } else if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_integer() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger {
            byte_offset,
            byte_count,
        }
    } else if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarFloat {
            byte_offset,
            byte_count,
        }
    } else if let Some((_, byte_offset, member_byte_count, members)) =
        operand.runtime_homogeneous_float_aggregate()
    {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeHomogeneousFloatAggregate {
            byte_offset,
            member_byte_count,
            members,
        }
    } else if operand.runtime_system_v_aggregate().is_some() {
        return Err(Diagnostic::error(
            "final AArch64 outbound-call replay retained a SysV-only aggregate operand",
        ));
    } else if let Some((_, byte_offset, byte_count, alignment)) = operand.runtime_small_aggregate()
    {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeSmallAggregate {
            byte_offset,
            byte_count,
            alignment,
        }
    } else if let Some((_, byte_offset, byte_count, alignment)) = operand.runtime_large_aggregate()
    {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeLargeAggregate {
            byte_offset,
            byte_count,
            alignment,
        }
    } else if let Some((_, byte_offset)) = operand.runtime_storage_address() {
        omega_isa_aarch64::Aarch64CallOperand::RuntimeStorageAddress { byte_offset }
    } else if let Some(value) = operand.immediate_integer() {
        omega_isa_aarch64::Aarch64CallOperand::ImmediateInteger(value)
    } else if let Some(value) = operand.byte_length() {
        omega_isa_aarch64::Aarch64CallOperand::ByteLength(value)
    } else {
        return Err(Diagnostic::error(
            "final outbound-call replay retained an unsupported parameter",
        ));
    })
}

fn outbound_relocated_operand_region(
    operand: &omega_target_operations::InstructionOperand,
) -> Option<omega_target_operations::RuntimeStorageRegion> {
    use omega_target_operations::InstructionOperandLike;

    operand
        .runtime_scalar_integer()
        .map(|(region, _, _)| region)
        .or_else(|| operand.runtime_scalar_float().map(|(region, _, _)| region))
        .or_else(|| {
            operand
                .runtime_homogeneous_float_aggregate()
                .map(|(region, _, _, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_system_v_aggregate()
                .map(|(region, _, _, _, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_small_aggregate()
                .map(|(region, _, _, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_large_aggregate()
                .map(|(region, _, _, _)| region)
        })
        .or_else(|| operand.runtime_storage_address().map(|(region, _)| region))
        .or_else(|| operand.runtime_string_pointer().map(|(region, _)| region))
        .or_else(|| operand.runtime_string_length().map(|(region, _)| region))
        .or_else(|| {
            operand
                .runtime_pointee_string_pointer()
                .map(|(region, _)| region)
        })
        .or_else(|| {
            operand
                .runtime_pointee_string_length()
                .map(|(region, _)| region)
        })
}

pub(super) fn encode_no_result_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<
    (
        Vec<u8>,
        usize,
        Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    ),
    Diagnostic,
> {
    use omega_target_operations::InstructionOperandLike;

    let get_std_handle = matches!(
        operation_key.operation,
        omega_calling_conventions::HostOperation::GetStdHandle
    );
    if (plan.result.is_some() && !get_std_handle)
        || plan.parameters.len() != operands.len()
        || operands.is_empty()
        || !operands.iter().all(|operand| {
            operand.immediate_integer().is_some() || operand.runtime_scalar_integer().is_some()
        })
    {
        return Err(Diagnostic::error(
            "final no-result import replay requires non-empty immediate/runtime-scalar operands",
        ));
    }
    let (inner, inner_call_site, inner_storage_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 no-result import replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let storage_sites = operands
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    operand.runtime_scalar_integer().map(|(region, _, _)| {
                        omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                            plan.policy,
                            operation_key,
                            operands,
                            index,
                            plan,
                        )
                        .map(|site| (site.byte_offset, region))
                    })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 storage-import replay lost a retained-plan operand site",
                    )
                })?;
            (bytes, site, storage_sites)
        }
        Architecture::Aarch64 => {
            let call_operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            let site = call_operands
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>()
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let bytes =
                omega_isa_aarch64::encode_host_call_sequence(&call_operands, &plan.parameters)?;
            let storage_sites = operands
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    operand.runtime_scalar_integer().map(|(region, _, _)| {
                        let site = call_operands
                            .iter()
                            .take(index)
                            .map(omega_isa_aarch64::operand_width)
                            .sum::<usize>()
                            + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                                &plan.parameters,
                                index,
                            );
                        (site, region)
                    })
                })
                .collect::<Vec<_>>();
            (bytes, site, storage_sites)
        }
    };
    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_storage_sites
            .into_iter()
            .map(|(site, region)| (prefix_width + site, region))
            .collect(),
    ))
}

pub(super) fn encode_integer_result_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<
    (
        Vec<u8>,
        usize,
        Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    ),
    Diagnostic,
> {
    use omega_target_operations::InstructionOperandLike;

    let result = plan.result.as_ref().ok_or_else(|| {
        Diagnostic::error("final immediate-result import replay lost its result plan")
    })?;
    let Some((result_region, _, _)) = operands
        .first()
        .and_then(InstructionOperandLike::runtime_scalar_integer)
    else {
        return Err(Diagnostic::error(
            "final immediate-result import replay lost its scalar result storage",
        ));
    };
    let win64_out_parameter = architecture == Architecture::X86_64
        && operation_key.capability == omega_calling_conventions::HostCapability::Clock
        && matches!(
            operation_key.operation,
            omega_calling_conventions::HostOperation::MonotonicTicks
                | omega_calling_conventions::HostOperation::MonotonicTicksPerSecond
                | omega_calling_conventions::HostOperation::WallClockRaw
        );
    if !matches!(
        result.shape.class,
        omega_calling_conventions::ValueClass::Integer
    ) || (!win64_out_parameter && plan.parameters.len() + 1 != operands.len())
        || (win64_out_parameter && operands.len() != 1)
        || !operands[1..].iter().all(|operand| {
            operand.immediate_integer().is_some() || operand.runtime_scalar_integer().is_some()
        })
    {
        return Err(Diagnostic::error(
            "final integer-result import replay requires one integer result and immediate/runtime-scalar arguments",
        ));
    }
    let (inner, inner_call_site, inner_storage_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let call_site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 immediate-result import replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let result_site = omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                0,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 immediate-result import replay has no retained-plan result site",
                )
            })?
            .byte_offset;
            let storage_sites = operands
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    operand.runtime_scalar_integer().map(|(region, _, _)| {
                        omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                            plan.policy,
                            operation_key,
                            operands,
                            index,
                            plan,
                        )
                        .map(|site| (site.byte_offset, region))
                    })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 integer-result import replay lost a retained-plan storage site",
                    )
                })?;
            if !storage_sites
                .iter()
                .any(|(site, region)| *site == result_site && *region == result_region)
            {
                return Err(Diagnostic::error(
                    "final x86 integer-result import replay lost its result storage site",
                ));
            }
            (bytes, call_site, storage_sites)
        }
        Architecture::Aarch64 => {
            let call_operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            let argument_width = call_operands[1..]
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>();
            let call_site = argument_width
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let result_site = argument_width
                + omega_isa_aarch64::host_call_stack_total_width_for_placements(&plan.parameters)
                + 4
                + usize::from(operation_key.dereferences_result()) * 4;
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: result_register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] = result.locations.as_slice()
            else {
                return Err(Diagnostic::error(
                    "final AArch64 immediate-result import replay requires one direct result register",
                ));
            };
            if usize::from(*byte_size) != usize::from(result.shape.byte_size) {
                return Err(Diagnostic::error(
                    "final AArch64 immediate-result import replay retained a partial result placement",
                ));
            }
            let bytes = if operation_key.dereferences_result() {
                omega_isa_aarch64::encode_host_call_sequence_value_returning_deref_from_operands(
                    call_operands.iter().copied(),
                    &plan.parameters,
                    *result_register,
                    usize::from(result.shape.byte_size),
                )?
            } else {
                omega_isa_aarch64::encode_host_call_sequence_value_returning_from_operands(
                    call_operands.iter().copied(),
                    &plan.parameters,
                    *result_register,
                    usize::from(result.shape.byte_size),
                )?
            };
            let mut storage_sites = vec![(result_site, result_region)];
            storage_sites.extend(operands[1..].iter().enumerate().filter_map(
                |(parameter_index, operand)| {
                    operand.runtime_scalar_integer().map(|(region, _, _)| {
                        let site = call_operands[1..1 + parameter_index]
                            .iter()
                            .map(omega_isa_aarch64::operand_width)
                            .sum::<usize>()
                            + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                                &plan.parameters,
                                parameter_index,
                            );
                        (site, region)
                    })
                },
            ));
            (bytes, call_site, storage_sites)
        }
    };
    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_storage_sites
            .into_iter()
            .map(|(site, region)| (prefix_width + site, region))
            .collect(),
    ))
}

pub(super) fn encode_scalar_parameter_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, usize, Vec<(usize, OutboundCallRelocationTarget)>), Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    let win64_composite_io = architecture == Architecture::X86_64
        && matches!(
            (operation_key.capability, operation_key.operation),
            (
                omega_calling_conventions::HostCapability::Stdout
                    | omega_calling_conventions::HostCapability::Stderr,
                omega_calling_conventions::HostOperation::Write
                    | omega_calling_conventions::HostOperation::WriteFile
            ) | (
                omega_calling_conventions::HostCapability::Stdin,
                omega_calling_conventions::HostOperation::ReadFile
            )
        );
    let discards_native_result = operation_key.discards_native_result();
    let result_operand_count = if win64_composite_io || discards_native_result {
        0
    } else {
        usize::from(plan.result.is_some())
    };
    let arguments = operands.get(result_operand_count..).ok_or_else(|| {
        Diagnostic::error("final scalar-parameter import replay lost its result operand")
    })?;
    if operation_key.dereferences_result()
        || (plan.parameters.len() != arguments.len() && !win64_composite_io)
        || !arguments.iter().all(|operand| {
            operand.immediate_integer().is_some()
                || operand.runtime_scalar_integer().is_some()
                || operand.runtime_scalar_float().is_some()
                || operand.runtime_homogeneous_float_aggregate().is_some()
                || operand.runtime_system_v_aggregate().is_some()
                || operand.runtime_small_aggregate().is_some()
                || operand.runtime_large_aggregate().is_some()
                || operand.data_address().is_some()
                || operand.runtime_storage_address().is_some()
                || operand.runtime_string_pointer().is_some()
                || operand.runtime_string_length().is_some()
                || operand.runtime_pointee_string_pointer().is_some()
                || operand.runtime_pointee_string_length().is_some()
                || operand.byte_length().is_some()
        })
        || (!win64_composite_io
            && !discards_native_result
            && plan
                .result
                .as_ref()
                .is_some_and(|result| match result.shape.class {
                    omega_calling_conventions::ValueClass::Integer => operands
                        .first()
                        .and_then(InstructionOperandLike::runtime_scalar_integer)
                        .is_none(),
                    // Result storage is an addressable scalar slot. Some
                    // frontend paths preserve its integer carrier spelling
                    // even when the selected CallPlan returns floating bits;
                    // the plan, not that storage label, chooses the foreign
                    // result register and store encoding.
                    omega_calling_conventions::ValueClass::Float => operands
                        .first()
                        .and_then(|operand| {
                            operand
                                .runtime_scalar_float()
                                .or_else(|| operand.runtime_scalar_integer())
                        })
                        .is_none(),
                    _ => true,
                }))
    {
        return Err(Diagnostic::error(
            "final scalar-parameter import replay requires scalar/data-address arguments and at most one direct scalar result",
        ));
    }

    let mut retained_data_symbols = data_symbols.iter();
    let (inner, inner_call_site, inner_address_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let call_site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 data-parameter import replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let mut address_sites = Vec::new();
            for (index, operand) in operands.iter().enumerate() {
                let target = if let Some(region) = outbound_relocated_operand_region(operand) {
                    OutboundCallRelocationTarget::Storage(region)
                } else if operand.data_address().is_some() {
                    OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                        retained_data_symbols.next().ok_or_else(|| {
                            Diagnostic::error(
                                "final x86 data-parameter import replay lost a data-object symbol",
                            )
                        })?,
                    ))
                } else {
                    continue;
                };
                let site = omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                    plan.policy,
                    operation_key,
                    operands,
                    index,
                    plan,
                )
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 data-parameter import replay lost an operand relocation site",
                    )
                })?
                .byte_offset;
                address_sites.push((site, target));
            }
            (bytes, call_site, address_sites)
        }
        Architecture::Aarch64 => {
            let call_operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            let argument_operands = &call_operands[result_operand_count..];
            let argument_width = argument_operands
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>();
            let call_site = argument_width
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let bytes = if discards_native_result {
                omega_isa_aarch64::encode_host_call_sequence(argument_operands, &plan.parameters)?
            } else if let Some(result) = plan.result.as_ref() {
                let [
                    omega_calling_conventions::ValueLocation::Register {
                        register: result_register,
                        value_byte_offset: 0,
                        byte_size,
                    },
                ] = result.locations.as_slice()
                else {
                    return Err(Diagnostic::error(
                        "final AArch64 data-parameter import replay requires one direct result register",
                    ));
                };
                if usize::from(*byte_size) != usize::from(result.shape.byte_size) {
                    return Err(Diagnostic::error(
                        "final AArch64 data-parameter import replay retained a partial result placement",
                    ));
                }
                match result.shape.class {
                    omega_calling_conventions::ValueClass::Integer => {
                        omega_isa_aarch64::encode_host_call_sequence_value_returning_from_operands(
                            call_operands.iter().copied(),
                            &plan.parameters,
                            *result_register,
                            usize::from(result.shape.byte_size),
                        )?
                    }
                    omega_calling_conventions::ValueClass::Float => {
                        if operands[0].runtime_scalar_integer().is_some() {
                            omega_isa_aarch64::encode_host_call_sequence_value_returning_float_from_operands(
                                call_operands.iter().copied(),
                                &plan.parameters,
                                *result_register,
                                usize::from(result.shape.byte_size),
                            )?
                        } else {
                            omega_isa_aarch64::encode_host_call_sequence_authored_float_returning_from_operands(
                                call_operands.iter().copied(),
                                &plan.parameters,
                                *result_register,
                            )?
                        }
                    }
                    _ => unreachable!("validated scalar result class"),
                }
            } else {
                omega_isa_aarch64::encode_host_call_sequence(argument_operands, &plan.parameters)?
            };
            let mut address_sites = Vec::new();
            if result_operand_count == 1 {
                let (result_region, _, _) = operands[0]
                    .runtime_scalar_integer()
                    .or_else(|| operands[0].runtime_scalar_float())
                    .expect("validated direct scalar result");
                let result_site = argument_width
                    + omega_isa_aarch64::host_call_stack_total_width_for_placements(
                        &plan.parameters,
                    )
                    + 4
                    + usize::from(plan.result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Float
                        )
                    })) * 4;
                address_sites.push((
                    result_site,
                    OutboundCallRelocationTarget::Storage(result_region),
                ));
            }
            for (parameter_index, operand) in arguments.iter().enumerate() {
                let target = if let Some(region) = outbound_relocated_operand_region(operand) {
                    OutboundCallRelocationTarget::Storage(region)
                } else if operand.data_address().is_some() {
                    OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                        retained_data_symbols.next().ok_or_else(|| {
                            Diagnostic::error(
                                "final AArch64 data-parameter import replay lost a data-object symbol",
                            )
                        })?,
                    ))
                } else {
                    continue;
                };
                let site = argument_operands
                    .iter()
                    .take(parameter_index)
                    .map(omega_isa_aarch64::operand_width)
                    .sum::<usize>()
                    + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                        &plan.parameters,
                        parameter_index,
                    );
                address_sites.push((site, target));
            }
            (bytes, call_site, address_sites)
        }
    };
    if retained_data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final scalar-parameter import replay retained an extra data-object symbol",
        ));
    }

    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_address_sites
            .into_iter()
            .map(|(site, target)| (prefix_width + site, target))
            .collect(),
    ))
}

pub(super) fn encode_authored_aggregate_result_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, usize, Vec<(usize, OutboundCallRelocationTarget)>), Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    let result = plan.result.as_ref().ok_or_else(|| {
        Diagnostic::error("final authored aggregate-result replay lost its result plan")
    })?;
    let Some(result_operand) = operands.first() else {
        return Err(Diagnostic::error(
            "final authored aggregate-result replay lost its result operand",
        ));
    };
    let arguments = &operands[1..];
    let result_is_aggregate = result_operand
        .runtime_homogeneous_float_aggregate()
        .is_some()
        || result_operand.runtime_system_v_aggregate().is_some()
        || result_operand.runtime_small_aggregate().is_some()
        || result_operand.runtime_large_aggregate().is_some();
    if !result_is_aggregate
        || plan.parameters.len() != arguments.len()
        || !arguments.iter().all(|operand| {
            operand.immediate_integer().is_some()
                || outbound_relocated_operand_region(operand).is_some()
                || operand.data_address().is_some()
        })
    {
        return Err(Diagnostic::error(
            "final authored aggregate-result replay requires one aggregate result and closed scalar/aggregate/data arguments",
        ));
    }

    let mut retained_data_symbols = data_symbols.iter();
    let (inner, inner_call_site, inner_address_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let call_site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 authored aggregate-result replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let mut address_sites = Vec::new();
            for (index, operand) in operands.iter().enumerate() {
                let target = if let Some(region) = outbound_relocated_operand_region(operand) {
                    OutboundCallRelocationTarget::Storage(region)
                } else if operand.data_address().is_some() {
                    OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                        retained_data_symbols.next().ok_or_else(|| {
                            Diagnostic::error(
                                "final x86 authored aggregate-result replay lost a data-object symbol",
                            )
                        })?,
                    ))
                } else {
                    continue;
                };
                let site = omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                    plan.policy,
                    operation_key,
                    operands,
                    index,
                    plan,
                )
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 authored aggregate-result replay lost an operand relocation site",
                    )
                })?
                .byte_offset;
                address_sites.push((site, target));
            }
            (bytes, call_site, address_sites)
        }
        Architecture::Aarch64 => {
            let call_operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            let lowered_result = call_operands[0];
            let argument_operands = &call_operands[1..];
            let result_prefix =
                omega_isa_aarch64::indirect_result_address_width(lowered_result).unwrap_or(0);
            let argument_width = argument_operands
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>();
            let call_site = result_prefix
                + argument_width
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let bytes = match result.shape.class {
                omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { .. } => {
                    omega_isa_aarch64::encode_host_call_sequence_hfa_returning_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        result,
                    )?
                }
                omega_calling_conventions::ValueClass::Integer
                    if result.shape.byte_size > 16 =>
                {
                    omega_isa_aarch64::encode_host_call_sequence_indirect_returning_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        result,
                    )?
                }
                omega_calling_conventions::ValueClass::Integer
                    if result.shape.byte_size > 8 =>
                {
                    omega_isa_aarch64::encode_host_call_sequence_small_aggregate_returning_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        result,
                    )?
                }
                _ => {
                    return Err(Diagnostic::error(
                        "final AArch64 authored aggregate-result replay retained an unsupported result class",
                    ));
                }
            };
            let result_region = outbound_relocated_operand_region(result_operand).ok_or_else(|| {
                Diagnostic::error(
                    "final AArch64 authored aggregate-result replay lost its result storage root",
                )
            })?;
            let result_site = if result_prefix == 0 {
                argument_width
                    + omega_isa_aarch64::host_call_stack_total_width_for_placements(
                        &plan.parameters,
                    )
                    + 4
            } else {
                0
            };
            let mut address_sites = vec![(
                result_site,
                OutboundCallRelocationTarget::Storage(result_region),
            )];
            for (parameter_index, operand) in arguments.iter().enumerate() {
                let target = if let Some(region) = outbound_relocated_operand_region(operand) {
                    OutboundCallRelocationTarget::Storage(region)
                } else if operand.data_address().is_some() {
                    OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                        retained_data_symbols.next().ok_or_else(|| {
                            Diagnostic::error(
                                "final AArch64 authored aggregate-result replay lost a data-object symbol",
                            )
                        })?,
                    ))
                } else {
                    continue;
                };
                let site = result_prefix
                    + argument_operands
                        .iter()
                        .take(parameter_index)
                        .map(omega_isa_aarch64::operand_width)
                        .sum::<usize>()
                    + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                        &plan.parameters,
                        parameter_index,
                    );
                address_sites.push((site, target));
            }
            (bytes, call_site, address_sites)
        }
    };
    if retained_data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final authored aggregate-result replay retained an extra data-object symbol",
        ));
    }

    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_address_sites
            .into_iter()
            .map(|(site, target)| (prefix_width + site, target))
            .collect(),
    ))
}

pub(super) fn encode_open_create_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, usize, Vec<(usize, OutboundCallRelocationTarget)>), Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    if architecture != Architecture::Aarch64
        || !matches!(
            (operation_key.capability, operation_key.operation),
            (
                omega_calling_conventions::HostCapability::Filesystem,
                omega_calling_conventions::HostOperation::OpenCreate
            )
        )
    {
        return Err(Diagnostic::error(
            "final open-create replay requires the Darwin AArch64 adapter",
        ));
    }
    let [result_operand, path, flags, mode] = operands else {
        return Err(Diagnostic::error(
            "final open-create replay requires result, path, flags, and mode operands",
        ));
    };
    let Some((result_region, _, _)) = result_operand.runtime_scalar_integer() else {
        return Err(Diagnostic::error(
            "final open-create replay lost its scalar result storage",
        ));
    };
    if !(path.data_address().is_some()
        || path.runtime_string_pointer().is_some()
        || path.runtime_pointee_string_pointer().is_some()
        || path.runtime_storage_address().is_some())
        || !(flags.immediate_integer().is_some() || flags.runtime_scalar_integer().is_some())
        || mode.immediate_integer().is_none()
        || plan.parameters.len() != 3
    {
        return Err(Diagnostic::error(
            "final open-create replay retained an incompatible concrete adapter shape",
        ));
    }
    let result = plan
        .result
        .as_ref()
        .ok_or_else(|| Diagnostic::error("final open-create replay lost its result placement"))?;
    let [
        omega_calling_conventions::ValueLocation::Register {
            register: result_register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = result.locations.as_slice()
    else {
        return Err(Diagnostic::error(
            "final open-create replay requires one direct result register",
        ));
    };
    if usize::from(*byte_size) != usize::from(result.shape.byte_size) {
        return Err(Diagnostic::error(
            "final open-create replay retained a partial result placement",
        ));
    }

    let call_operands = operands
        .iter()
        .map(aarch64_outbound_syscall_operand)
        .collect::<Result<Vec<_>, _>>()?;
    let argument_operands = &call_operands[1..];
    let argument_width = argument_operands
        .iter()
        .map(omega_isa_aarch64::operand_width)
        .sum::<usize>();
    let call_site = argument_width
        + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
            &plan.parameters,
            plan.parameters.len(),
        );
    let inner =
        omega_isa_aarch64::encode_host_call_sequence_value_returning_open_create_from_operands(
            call_operands.iter().copied(),
            &plan.parameters,
            *result_register,
            usize::from(result.shape.byte_size),
        )?;
    let result_site = argument_width
        + omega_isa_aarch64::host_call_stack_total_width_for_placements(&plan.parameters)
        + 4;
    let mut address_sites = vec![(
        result_site,
        OutboundCallRelocationTarget::Storage(result_region),
    )];
    let mut retained_data_symbols = data_symbols.iter();
    for (parameter_index, operand) in operands[1..].iter().enumerate() {
        let storage_region = outbound_relocated_operand_region(operand)
            .or_else(|| operand.runtime_string_pointer().map(|(region, _)| region))
            .or_else(|| {
                operand
                    .runtime_pointee_string_pointer()
                    .map(|(region, _)| region)
            })
            .or_else(|| operand.runtime_storage_address().map(|(region, _)| region));
        let target = if let Some(region) = storage_region {
            OutboundCallRelocationTarget::Storage(region)
        } else if operand.data_address().is_some() {
            OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                retained_data_symbols.next().ok_or_else(|| {
                    Diagnostic::error("final open-create replay lost its path data symbol")
                })?,
            ))
        } else {
            continue;
        };
        let site = argument_operands
            .iter()
            .take(parameter_index)
            .map(omega_isa_aarch64::operand_width)
            .sum::<usize>()
            + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                &plan.parameters,
                parameter_index,
            );
        address_sites.push((site, target));
    }
    if retained_data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final open-create replay retained an extra path data symbol",
        ));
    }

    let mut bytes = omega_isa_aarch64::encode_foreign_float_control_prefix_bytes().to_vec();
    let prefix_width = omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH;
    bytes.extend(inner);
    bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes());
    Ok((
        bytes,
        prefix_width + call_site,
        address_sites
            .into_iter()
            .map(|(site, target)| (prefix_width + site, target))
            .collect(),
    ))
}

pub(super) fn encode_float_parameter_result_import(
    architecture: Architecture,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: &[omega_target_operations::InstructionOperand],
    plan: &omega_calling_conventions::CallPlan,
) -> Result<
    (
        Vec<u8>,
        usize,
        Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    ),
    Diagnostic,
> {
    use omega_target_operations::InstructionOperandLike;

    let result = plan.result.as_ref().ok_or_else(|| {
        Diagnostic::error("final float-parameter import replay lost its result plan")
    })?;
    let Some((result_region, result_offset, result_byte_size)) = operands
        .first()
        .and_then(InstructionOperandLike::runtime_scalar_integer)
    else {
        return Err(Diagnostic::error(
            "final float-parameter import replay lost its scalar result storage",
        ));
    };
    if !matches!(
        result.shape.class,
        omega_calling_conventions::ValueClass::Integer
            | omega_calling_conventions::ValueClass::Float
    ) || plan.parameters.len() + 1 != operands.len()
        || operands[1..].is_empty()
        || !operands[1..]
            .iter()
            .all(|operand| operand.runtime_scalar_float().is_some())
    {
        return Err(Diagnostic::error(
            "final float-parameter import replay requires one scalar result and runtime-float arguments",
        ));
    }
    let (inner, inner_call_site, inner_storage_sites) = match architecture {
        Architecture::X86_64 => {
            let bytes = omega_isa_x86_64::encode_host_call_sequence_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )?;
            let call_site = omega_isa_x86_64::host_call_external_relocation_site_with_plan(
                plan.policy,
                operation_key,
                operands,
                plan,
            )
            .ok_or_else(|| {
                Diagnostic::error(
                    "final x86 float-parameter import replay has no retained-plan call site",
                )
            })?
            .byte_offset;
            let storage_sites = operands
                .iter()
                .enumerate()
                .map(|(index, operand)| {
                    let region = operand
                        .runtime_scalar_integer()
                        .map(|(region, _, _)| region)
                        .or_else(|| operand.runtime_scalar_float().map(|(region, _, _)| region))?;
                    omega_isa_x86_64::host_call_data_relocation_site_with_plan(
                        plan.policy,
                        operation_key,
                        operands,
                        index,
                        plan,
                    )
                    .map(|site| (site.byte_offset, region))
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    Diagnostic::error(
                        "final x86 float-parameter import replay lost a retained-plan storage site",
                    )
                })?;
            (bytes, call_site, storage_sites)
        }
        Architecture::Aarch64 => {
            let mut call_operands = Vec::with_capacity(operands.len());
            call_operands.push(
                omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger {
                    byte_offset: result_offset,
                    byte_count: result_byte_size,
                },
            );
            call_operands.extend(
                operands[1..]
                    .iter()
                    .map(aarch64_outbound_syscall_operand)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            let argument_width = call_operands[1..]
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum::<usize>();
            let call_site = argument_width
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                );
            let result_site = argument_width
                + omega_isa_aarch64::host_call_stack_total_width_for_placements(&plan.parameters)
                + 4
                + usize::from(matches!(
                    result.shape.class,
                    omega_calling_conventions::ValueClass::Float
                )) * 4;
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: result_register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] = result.locations.as_slice()
            else {
                return Err(Diagnostic::error(
                    "final AArch64 float-parameter import replay requires one direct result register",
                ));
            };
            if usize::from(*byte_size) != usize::from(result.shape.byte_size) {
                return Err(Diagnostic::error(
                    "final AArch64 float-parameter import replay retained a partial result placement",
                ));
            }
            let bytes = match result.shape.class {
                omega_calling_conventions::ValueClass::Integer => {
                    omega_isa_aarch64::encode_host_call_sequence_value_returning_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        *result_register,
                        usize::from(result.shape.byte_size),
                    )?
                }
                omega_calling_conventions::ValueClass::Float => {
                    omega_isa_aarch64::encode_host_call_sequence_value_returning_float_from_operands(
                        call_operands.iter().copied(),
                        &plan.parameters,
                        *result_register,
                        usize::from(result.shape.byte_size),
                    )?
                }
                _ => unreachable!("validated scalar result class"),
            };
            let mut storage_sites = vec![(result_site, result_region)];
            storage_sites.extend(operands[1..].iter().enumerate().map(
                |(parameter_index, operand)| {
                    let (region, _, _) = operand
                        .runtime_scalar_float()
                        .expect("validated runtime-float parameter");
                    let site = call_operands[1..1 + parameter_index]
                        .iter()
                        .map(omega_isa_aarch64::operand_width)
                        .sum::<usize>()
                        + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                            &plan.parameters,
                            parameter_index,
                        );
                    (site, region)
                },
            ));
            (bytes, call_site, storage_sites)
        }
    };
    let mut bytes = Vec::new();
    let prefix_width = match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_prefix_bytes());
            omega_isa_x86_64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_prefix_bytes());
            omega_isa_aarch64::FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH
        }
    };
    bytes.extend(inner);
    match architecture {
        Architecture::X86_64 => {
            bytes.extend(omega_isa_x86_64::encode_foreign_float_control_suffix_bytes())
        }
        Architecture::Aarch64 => {
            bytes.extend(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        }
    }
    Ok((
        bytes,
        prefix_width + inner_call_site,
        inner_storage_sites
            .into_iter()
            .map(|(site, region)| (prefix_width + site, region))
            .collect(),
    ))
}
#[cfg(test)]
pub(super) use indirect_calls::encode_aarch64_indirect_call_replay;
pub(super) use indirect_calls::encode_indirect_call_replay;
