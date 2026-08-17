//! Replays target-specific runtime imports, syscalls, and text boundaries.

use super::*;

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

fn aarch64_replay_scalar_result_register(
    result: &omega_calling_conventions::ValuePlacement,
    label: &str,
) -> Result<omega_calling_conventions::MachineRegister, Diagnostic> {
    use omega_calling_conventions::ValueLocation;

    match result.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                ..
            },
        ] => Ok(*register),
        locations => Err(Diagnostic::error(format!(
            "final AArch64 {label} result did not retain one scalar register: {locations:?}"
        ))),
    }
}

fn validate_aarch64_indirect_replay_plan(
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(), Diagnostic> {
    use omega_calling_conventions::{CallingPolicy, EntryControl, MachineRegister};

    if plan.policy != CallingPolicy::Aapcs64
        || plan.entry_control != EntryControl::CallReturn
        || plan.stack_alignment != 16
        || plan.shadow_bytes != 0
    {
        return Err(Diagnostic::error(format!(
            "final AArch64 indirect-call replay cannot realize plan policy={:?}, control={:?}, alignment={}, shadow_bytes={}",
            plan.policy, plan.entry_control, plan.stack_alignment, plan.shadow_bytes
        )));
    }
    for scratch in [
        MachineRegister::Aarch64X(0),
        MachineRegister::Aarch64X(9),
        MachineRegister::Aarch64X(10),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64V(31),
    ] {
        if !plan.ordinary_clobbers.contains(scratch) {
            return Err(Diagnostic::error(format!(
                "final AArch64 indirect-call replay scratch register {scratch:?} exceeds the retained plan's ordinary-clobber ceiling"
            )));
        }
    }
    Ok(())
}

pub(super) fn encode_aarch64_indirect_call_replay(
    operands: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
    mechanism: &omega_calling_conventions::HostBindingMechanism,
    plan: &omega_calling_conventions::CallPlan,
    result_present: bool,
) -> Result<(Vec<u8>, Vec<(usize, OutboundCallRelocationTarget)>), Diagnostic> {
    use omega_calling_conventions::{HostBindingMechanism, ValueClass, ValueLocation};
    use omega_target_operations::InstructionOperandLike;

    validate_aarch64_indirect_replay_plan(plan)?;
    if plan.result.is_some() != result_present {
        return Err(Diagnostic::error(
            "final AArch64 indirect-call result operand disagrees with its retained call plan",
        ));
    }
    let lowered = operands
        .iter()
        .map(aarch64_outbound_syscall_operand)
        .collect::<Result<Vec<_>, _>>()?;
    let result = plan.result.as_ref();
    let byte_offset = match mechanism {
        HostBindingMechanism::VtableSlot { index } => index
            .checked_mul(8)
            .and_then(|offset| usize::try_from(offset).ok())
            .ok_or_else(|| Diagnostic::error("final AArch64 vtable slot offset overflowed"))?,
        HostBindingMechanism::VtableField { byte_offset, .. }
        | HostBindingMechanism::TableFunction { byte_offset, .. } => *byte_offset,
        HostBindingMechanism::Import { .. } | HostBindingMechanism::Syscall { .. } => {
            return Err(Diagnostic::error(
                "final AArch64 indirect-call replay retained a direct-call mechanism",
            ));
        }
    };

    let passes_receiver = !matches!(mechanism, HostBindingMechanism::TableFunction { .. });
    if passes_receiver
        && !matches!(
            plan.parameters
                .first()
                .map(|placement| placement.locations.as_slice()),
            Some([ValueLocation::Register {
                register: omega_calling_conventions::MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 8,
            }])
        )
    {
        return Err(Diagnostic::error(
            "final AArch64 vtable replay requires one full-width receiver in x0",
        ));
    }

    let inner = if passes_receiver {
        match result.map(|result| result.shape.class) {
            None => omega_isa_aarch64::encode_vtable_call_sequence_at_offset_from_operands(
                lowered.iter().copied(),
                &plan.parameters,
                byte_offset,
            ),
            Some(ValueClass::Integer)
                if result.is_some_and(|result| result.shape.byte_size > 16) =>
            {
                omega_isa_aarch64::encode_vtable_call_sequence_at_offset_indirect_returning_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    result.expect("matched present result"),
                    byte_offset,
                )
            }
            Some(ValueClass::Integer)
                if result.is_some_and(|result| result.shape.byte_size > 8) =>
            {
                omega_isa_aarch64::encode_vtable_call_sequence_at_offset_small_aggregate_returning_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    result.expect("matched present result"),
                    byte_offset,
                )
            }
            Some(ValueClass::Integer) => {
                let result = result.expect("matched present result");
                omega_isa_aarch64::encode_vtable_call_sequence_at_offset_value_returning_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    aarch64_replay_scalar_result_register(result, "vtable integer")?,
                    byte_offset,
                )
            }
            Some(ValueClass::Float) => {
                let result = result.expect("matched present result");
                omega_isa_aarch64::encode_vtable_call_sequence_at_offset_float_returning_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    aarch64_replay_scalar_result_register(result, "vtable float")?,
                    byte_offset,
                )
            }
            Some(ValueClass::HomogeneousFloatAggregate { .. }) => {
                omega_isa_aarch64::encode_vtable_call_sequence_at_offset_hfa_returning_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    result.expect("matched present result"),
                    byte_offset,
                )
            }
            Some(ValueClass::SystemVAggregate { .. }) => Err(Diagnostic::error(
                "final AArch64 vtable replay retained a SysV aggregate result",
            )),
        }?
    } else {
        let table_index = usize::from(result_present);
        if !matches!(
            lowered.get(table_index),
            Some(omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger { byte_count: 8, .. })
        ) {
            return Err(Diagnostic::error(
                "final AArch64 table-function replay requires an eight-byte runtime table pointer",
            ));
        }
        match result.map(|result| result.shape.class) {
            None => omega_isa_aarch64::encode_table_function_call_sequence_from_operands(
                lowered.iter().copied(),
                &plan.parameters,
                None,
                byte_offset,
            ),
            Some(ValueClass::Integer)
                if result.is_some_and(|result| result.shape.byte_size > 16) =>
            {
                omega_isa_aarch64::encode_table_function_call_sequence_indirect_returning_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    result.expect("matched present result"),
                    byte_offset,
                )
            }
            Some(ValueClass::Integer)
                if result.is_some_and(|result| result.shape.byte_size > 8) =>
            {
                omega_isa_aarch64::encode_table_function_call_sequence_small_aggregate_returning_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    result.expect("matched present result"),
                    byte_offset,
                )
            }
            Some(ValueClass::Integer) => {
                let result = result.expect("matched present result");
                omega_isa_aarch64::encode_table_function_call_sequence_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    Some(aarch64_replay_scalar_result_register(
                        result,
                        "table-function integer",
                    )?),
                    byte_offset,
                )
            }
            Some(ValueClass::Float) => {
                let result = result.expect("matched present result");
                omega_isa_aarch64::encode_table_function_call_sequence_float_returning_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    aarch64_replay_scalar_result_register(result, "table-function float")?,
                    byte_offset,
                )
            }
            Some(ValueClass::HomogeneousFloatAggregate { .. }) => {
                omega_isa_aarch64::encode_table_function_call_sequence_hfa_returning_from_operands(
                    lowered.iter().copied(),
                    &plan.parameters,
                    result.expect("matched present result"),
                    byte_offset,
                )
            }
            Some(ValueClass::SystemVAggregate { .. }) => Err(Diagnostic::error(
                "final AArch64 table-function replay retained a SysV aggregate result",
            )),
        }?
    };

    let argument_start = if passes_receiver {
        usize::from(result_present)
    } else {
        usize::from(result_present) + 1
    };
    let table_index = usize::from(result_present);
    let result_prefix = if result_present {
        omega_isa_aarch64::indirect_result_address_width(lowered[0]).unwrap_or(0)
    } else {
        0
    };
    let argument_width = |end: usize| {
        lowered[argument_start..end]
            .iter()
            .map(omega_isa_aarch64::operand_width)
            .sum::<usize>()
    };
    let mut retained_data_symbols = data_symbols.iter();
    let mut address_sites = Vec::new();
    for (operand_index, operand) in operands.iter().enumerate() {
        let target = if let Some(region) = outbound_relocated_operand_region(operand) {
            OutboundCallRelocationTarget::Storage(region)
        } else if operand.data_address().is_some() {
            OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                retained_data_symbols.next().ok_or_else(|| {
                    Diagnostic::error(
                        "final AArch64 indirect-call replay lost a retained data-object symbol",
                    )
                })?,
            ))
        } else {
            continue;
        };
        let site = if result_present && operand_index == 0 {
            if matches!(
                lowered[0],
                omega_isa_aarch64::Aarch64CallOperand::RuntimeLargeAggregate { .. }
            ) {
                0
            } else {
                let float_result_move = usize::from(matches!(
                    lowered[0],
                    omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarFloat { .. }
                )) * 4;
                let dispatch_operand_width = if passes_receiver {
                    0
                } else {
                    omega_isa_aarch64::operand_width(&lowered[table_index])
                };
                argument_width(lowered.len())
                    + dispatch_operand_width
                    + omega_isa_aarch64::host_call_stack_total_width_for_placements(
                        &plan.parameters,
                    )
                    + 8
                    + float_result_move
            }
        } else if !passes_receiver && operand_index == table_index {
            result_prefix
                + argument_width(lowered.len())
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    plan.parameters.len(),
                )
        } else if operand_index >= argument_start {
            result_prefix
                + argument_width(operand_index)
                + omega_isa_aarch64::host_call_stack_prefix_width_for_placements(
                    &plan.parameters,
                    operand_index - argument_start,
                )
        } else {
            return Err(Diagnostic::error(
                "final AArch64 indirect-call replay could not place an address operand",
            ));
        };
        address_sites.push((site, target));
    }
    if retained_data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final AArch64 indirect-call replay retained unused data-object symbols",
        ));
    }

    let prefix = omega_isa_aarch64::encode_foreign_float_control_prefix_bytes();
    let suffix = omega_isa_aarch64::encode_foreign_float_control_suffix_bytes();
    let mut bytes = Vec::with_capacity(prefix.len() + inner.len() + suffix.len());
    bytes.extend(prefix);
    bytes.extend(inner);
    bytes.extend(suffix);
    for (site, _) in &mut address_sites {
        *site += prefix.len();
    }
    Ok((bytes, address_sites))
}

pub(super) fn encode_indirect_call_replay(
    architecture: Architecture,
    operands: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
    mechanism: &omega_calling_conventions::HostBindingMechanism,
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, Vec<(usize, OutboundCallRelocationTarget)>), Diagnostic> {
    use omega_calling_conventions::{CallingPolicy, HostBindingMechanism};
    use omega_target_operations::InstructionOperandLike;

    let dispatch_only = usize::from(matches!(
        mechanism,
        HostBindingMechanism::TableFunction { .. }
    ));
    let parameter_count = plan
        .parameters
        .len()
        .checked_add(dispatch_only)
        .ok_or_else(|| Diagnostic::error("final indirect-call operand count overflowed"))?;
    let result_present = operands.len() == parameter_count + 1;
    if operands.is_empty() || (operands.len() != parameter_count && !result_present) {
        return Err(Diagnostic::error(
            "final indirect-call replay retained an operand count incompatible with its call plan",
        ));
    }
    if matches!(mechanism, HostBindingMechanism::VtableSlot { .. }) && result_present {
        return Err(Diagnostic::error(
            "final slot-indexed vtable replay unexpectedly retained a result operand",
        ));
    }

    if architecture == Architecture::Aarch64 {
        return encode_aarch64_indirect_call_replay(
            operands,
            data_symbols,
            mechanism,
            plan,
            result_present,
        );
    }

    let field_offset = match mechanism {
        HostBindingMechanism::VtableSlot { index } => index
            .checked_mul(8)
            .ok_or_else(|| Diagnostic::error("final vtable slot offset overflowed"))?,
        HostBindingMechanism::VtableField { byte_offset, .. }
        | HostBindingMechanism::TableFunction { byte_offset, .. } => i64::try_from(*byte_offset)
            .map_err(|_| Diagnostic::error("final indirect-call field offset overflowed"))?,
        HostBindingMechanism::Import { .. } | HostBindingMechanism::Syscall { .. } => {
            return Err(Diagnostic::error(
                "final indirect-call replay retained a direct-call mechanism",
            ));
        }
    };

    let (inner, raw_sites) = match (plan.policy, mechanism) {
        (CallingPolicy::MicrosoftX64, HostBindingMechanism::VtableSlot { index }) => (
            omega_isa_x86_64::encode_win64_vtable_call_with_plan(operands, *index, plan)?,
            omega_isa_x86_64::win64_vtable_call_relocation_sites_with_plan(operands, false, plan),
        ),
        (CallingPolicy::MicrosoftX64, HostBindingMechanism::VtableField { .. }) => (
            omega_isa_x86_64::encode_win64_vtable_call_at_offset_with_plan(
                operands,
                field_offset,
                result_present,
                plan,
            )?,
            omega_isa_x86_64::win64_vtable_call_relocation_sites_with_plan(
                operands,
                result_present,
                plan,
            ),
        ),
        (CallingPolicy::MicrosoftX64, HostBindingMechanism::TableFunction { .. }) => (
            omega_isa_x86_64::encode_win64_table_function_call_with_plan(
                operands,
                field_offset,
                result_present,
                plan,
            )?,
            omega_isa_x86_64::win64_table_function_call_relocation_sites_with_plan(
                operands,
                result_present,
                plan,
            ),
        ),
        (CallingPolicy::SystemVAMD64, HostBindingMechanism::VtableSlot { .. })
        | (CallingPolicy::SystemVAMD64, HostBindingMechanism::VtableField { .. }) => {
            let bytes = omega_isa_x86_64::encode_sysv_vtable_call_with_plan(
                operands,
                field_offset,
                result_present,
                plan,
            )?;
            let sites = operands
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    (outbound_relocated_operand_region(operand).is_some()
                        || operand.data_address().is_some())
                    .then_some(index)
                })
                .map(|index| {
                    let byte_offset =
                        omega_isa_x86_64::sysv_vtable_call_data_relocation_byte_offset_with_plan(
                            operands,
                            field_offset,
                            result_present,
                            index,
                            plan,
                        );
                    (byte_offset != 0)
                        .then_some(omega_isa_x86_64::X86_64RelocationSite {
                            operand_index: Some(index),
                            byte_offset,
                            byte_width: 8,
                            kind: omega_isa_x86_64::X86_64RelocationSiteKind::Absolute64,
                        })
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "final SysV vtable replay lost an operand relocation site",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            (bytes, sites)
        }
        (CallingPolicy::SystemVAMD64, HostBindingMechanism::TableFunction { .. }) => {
            let bytes = omega_isa_x86_64::encode_sysv_table_function_call_with_plan(
                operands,
                field_offset,
                result_present,
                plan,
            )?;
            let sites = operands
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    (outbound_relocated_operand_region(operand).is_some()
                        || operand.data_address().is_some())
                    .then_some(index)
                })
                .map(|index| {
                    let byte_offset = omega_isa_x86_64::sysv_table_function_call_data_relocation_byte_offset_with_plan(
                        operands,
                        field_offset,
                        result_present,
                        index,
                        plan,
                    );
                    (byte_offset != 0)
                        .then_some(omega_isa_x86_64::X86_64RelocationSite {
                            operand_index: Some(index),
                            byte_offset,
                            byte_width: 8,
                            kind: omega_isa_x86_64::X86_64RelocationSiteKind::Absolute64,
                        })
                        .ok_or_else(|| Diagnostic::error("final SysV table-function replay lost an operand relocation site"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            (bytes, sites)
        }
        _ => {
            return Err(Diagnostic::error(
                "final x86-64 indirect-call replay retained a non-x86 calling policy",
            ));
        }
    };

    let mut retained_data_symbols = data_symbols.iter();
    let mut address_sites = Vec::with_capacity(raw_sites.len());
    for site in raw_sites {
        let index = site.operand_index.ok_or_else(|| {
            Diagnostic::error("final indirect-call replay retained an unbound relocation site")
        })?;
        let operand = operands.get(index).ok_or_else(|| {
            Diagnostic::error("final indirect-call replay relocation index is out of bounds")
        })?;
        let target = if let Some(region) = outbound_relocated_operand_region(operand) {
            OutboundCallRelocationTarget::Storage(region)
        } else if operand.data_address().is_some() {
            OutboundCallRelocationTarget::Data(std::sync::Arc::clone(
                retained_data_symbols.next().ok_or_else(|| {
                    Diagnostic::error(
                        "final indirect-call replay lost a retained data-object symbol",
                    )
                })?,
            ))
        } else {
            return Err(Diagnostic::error(
                "final indirect-call replay retained a relocation for a non-address operand",
            ));
        };
        let site_start = site.byte_offset.checked_sub(2).ok_or_else(|| {
            Diagnostic::error("final indirect-call relocation site precedes its instruction")
        })?;
        address_sites.push((site_start, target));
    }
    if retained_data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final indirect-call replay retained unused data-object symbols",
        ));
    }

    let prefix = omega_isa_x86_64::encode_foreign_float_control_prefix_bytes();
    let suffix = omega_isa_x86_64::encode_foreign_float_control_suffix_bytes();
    let mut bytes = Vec::with_capacity(prefix.len() + inner.len() + suffix.len());
    bytes.extend(prefix);
    bytes.extend(inner);
    bytes.extend(suffix);
    for (site, _) in &mut address_sites {
        *site += prefix.len();
    }
    Ok((bytes, address_sites))
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
