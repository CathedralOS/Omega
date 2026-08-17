//! Replays outbound syscalls and derives their exact relocation targets.

use super::*;

fn outbound_syscall_operand_storage_region(
    operand: &omega_target_operations::InstructionOperand,
) -> Option<omega_target_operations::RuntimeStorageRegion> {
    use omega_target_operations::InstructionOperandLike;

    operand
        .runtime_string_pointer()
        .map(|(region, _)| region)
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
        .or_else(|| {
            operand
                .runtime_scalar_integer()
                .map(|(region, _, _)| region)
        })
        .or_else(|| operand.runtime_storage_address().map(|(region, _)| region))
}

pub(in crate::checked) fn outbound_syscall_argument_storage_sites(
    architecture: Architecture,
    arguments: &[omega_target_operations::InstructionOperand],
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let aarch64_arguments = (architecture == Architecture::Aarch64)
        .then(|| {
            arguments
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    arguments
        .iter()
        .enumerate()
        .filter_map(|(index, operand)| {
            outbound_syscall_operand_storage_region(operand).map(|region| {
                let site = match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::syscall_data_relocation_byte_offset(
                        arguments, index,
                    )
                    .checked_sub(2)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "x86 outbound syscall argument relocation precedes its address opcode",
                        )
                    })?,
                    Architecture::Aarch64 => aarch64_arguments
                        .as_ref()
                        .expect("AArch64 operands were retained")
                        .iter()
                        .take(index)
                        .map(omega_isa_aarch64::operand_width)
                        .sum(),
                };
                Ok((site, region))
            })
        })
        .collect()
}

pub(in crate::checked) fn outbound_syscall_argument_data_sites(
    architecture: Architecture,
    arguments: &[omega_target_operations::InstructionOperand],
    data_symbols: &[std::sync::Arc<str>],
) -> Result<Vec<(usize, std::sync::Arc<str>)>, Diagnostic> {
    use omega_target_operations::InstructionOperandLike;

    let aarch64_arguments = (architecture == Architecture::Aarch64)
        .then(|| {
            arguments
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let mut data_symbols = data_symbols.iter();
    let mut sites = Vec::new();
    for (index, operand) in arguments.iter().enumerate() {
        if operand.data_address().is_none() {
            continue;
        }
        let Some(symbol) = data_symbols.next() else {
            return Err(Diagnostic::error(
                "final outbound syscall replay lost a data-object symbol",
            ));
        };
        let site = match architecture {
            Architecture::X86_64 => {
                omega_isa_x86_64::syscall_data_relocation_byte_offset(arguments, index)
                    .checked_sub(2)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "x86 outbound syscall argument relocation precedes its address opcode",
                        )
                    })?
            }
            Architecture::Aarch64 => aarch64_arguments
                .as_ref()
                .expect("AArch64 operands were retained")
                .iter()
                .take(index)
                .map(omega_isa_aarch64::operand_width)
                .sum(),
        };
        sites.push((site, std::sync::Arc::clone(symbol)));
    }
    if data_symbols.next().is_some() {
        return Err(Diagnostic::error(
            "final outbound syscall replay retained an extra data-object symbol",
        ));
    }
    Ok(sites)
}

pub(in crate::checked) fn outbound_syscall_data_relocation_targets(
    storage_sites: Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    data_sites: Vec<(usize, std::sync::Arc<str>)>,
) -> Vec<(usize, OutboundCallRelocationTarget)> {
    let mut sites = storage_sites
        .into_iter()
        .map(|(site, region)| (site, OutboundCallRelocationTarget::Storage(region)))
        .chain(
            data_sites
                .into_iter()
                .map(|(site, symbol)| (site, OutboundCallRelocationTarget::Data(symbol))),
        )
        .collect::<Vec<_>>();
    sites.sort_unstable_by_key(|(site, _)| *site);
    sites
}

pub(super) struct OutboundSyscallReplayRegisters {
    pub(super) parameters: Vec<omega_calling_conventions::MachineRegister>,
    pub(super) result: omega_calling_conventions::MachineRegister,
    pub(super) number: omega_calling_conventions::MachineRegister,
    pub(super) immediate: u16,
}

pub(super) fn outbound_syscall_replay_registers(
    architecture: Architecture,
    plan: &omega_calling_conventions::CallPlan,
    parameter_count: usize,
) -> Result<OutboundSyscallReplayRegisters, Diagnostic> {
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, EntryControl, MachineRegister, ValueLocation, ValueShape,
    };

    let EntryControl::SupervisorCall {
        number_register,
        immediate,
    } = plan.entry_control
    else {
        return Err(Diagnostic::error(
            "final composite syscall replay retained non-supervisor entry control",
        ));
    };
    let word = ValueShape::integer(8, 8);
    omega_calling_conventions::validate_call_plan(
        plan,
        &CallSignature {
            parameters: vec![word; parameter_count],
            result: Some(word),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "final composite syscall replay retained an incompatible normalized plan: {error}"
        ))
    })?;
    let expected_policy = match architecture {
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
    };
    if plan.policy != expected_policy
        || plan.stack_alignment != 16
        || plan.shadow_bytes != 0
        || (architecture == Architecture::X86_64 && immediate != 0)
        || plan.parameters.len() != parameter_count
    {
        return Err(Diagnostic::error(
            "final composite syscall replay retained incompatible policy, stack, or arity",
        ));
    }
    let fixed_scratch = match architecture {
        Architecture::X86_64 => &[MachineRegister::X86Rax, MachineRegister::X86R11][..],
        Architecture::Aarch64 => &[][..],
    };
    if fixed_scratch
        .iter()
        .copied()
        .chain([number_register])
        .any(|register| !plan.ordinary_clobbers.contains(register))
    {
        return Err(Diagnostic::error(
            "final composite syscall replay exceeds its ordinary-clobber ceiling",
        ));
    }
    let parameters = plan
        .parameters
        .iter()
        .map(|placement| match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] => Ok(*register),
            _ => Err(Diagnostic::error(
                "final composite syscall replay requires one full-word register per parameter",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = match plan
        .result
        .as_ref()
        .map(|placement| placement.locations.as_slice())
    {
        Some(
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ],
        ) => *register,
        _ => {
            return Err(Diagnostic::error(
                "final composite syscall replay requires one full-word result register",
            ));
        }
    };
    Ok(OutboundSyscallReplayRegisters {
        parameters,
        result,
        number: number_register,
        immediate,
    })
}

pub(in crate::checked) fn encode_linux_timespec_result_outbound_syscall(
    architecture: Architecture,
    operands: &[omega_target_operations::InstructionOperand],
    number: u32,
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    let registers = outbound_syscall_replay_registers(architecture, plan, 2)?;
    match architecture {
        Architecture::X86_64 => {
            let (bytes, site) = omega_isa_x86_64::encode_linux_timespec_syscall(
                operands,
                number,
                &registers.parameters,
                registers.result,
                registers.number,
                registers.immediate,
            )?;
            Ok((bytes, site.byte_offset))
        }
        Architecture::Aarch64 => {
            let operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            omega_isa_aarch64::encode_linux_timespec_syscall(
                &operands,
                number,
                &registers.parameters,
                registers.result,
                registers.number,
                registers.immediate,
            )
        }
    }
}

pub(in crate::checked) fn encode_linux_timespec_argument_outbound_syscall(
    architecture: Architecture,
    operands: &[omega_target_operations::InstructionOperand],
    number: u32,
    plan: &omega_calling_conventions::CallPlan,
) -> Result<(Vec<u8>, Option<usize>), Diagnostic> {
    let registers = outbound_syscall_replay_registers(architecture, plan, 2)?;
    match architecture {
        Architecture::X86_64 => {
            let (bytes, site) = omega_isa_x86_64::encode_linux_timespec_argument_syscall(
                operands,
                number,
                &registers.parameters,
                registers.result,
                registers.number,
                registers.immediate,
            )?;
            Ok((bytes, site.map(|site| site.byte_offset)))
        }
        Architecture::Aarch64 => {
            let operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            omega_isa_aarch64::encode_linux_timespec_argument_syscall(
                &operands,
                number,
                &registers.parameters,
                registers.result,
                registers.number,
                registers.immediate,
            )
        }
    }
}

pub(in crate::checked) fn encode_simple_outbound_syscall(
    architecture: Architecture,
    operands: &[omega_target_operations::InstructionOperand],
    number: u32,
    plan: &omega_calling_conventions::CallPlan,
) -> Result<
    (
        Vec<u8>,
        Vec<(usize, omega_target_operations::RuntimeStorageRegion)>,
    ),
    Diagnostic,
> {
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, EntryControl, MachineRegister, ValueLocation, ValueShape,
    };
    use omega_target_operations::InstructionOperandLike;

    let EntryControl::SupervisorCall {
        number_register,
        immediate,
    } = plan.entry_control
    else {
        return Err(Diagnostic::error(
            "final outbound syscall replay retained non-supervisor entry control",
        ));
    };
    // The retained ABI result does not imply a leading Omega result operand:
    // statement-shaped adapters may intentionally discard the native status.
    // Operand arity distinguishes those calls from value-producing syscalls.
    let has_result_operand =
        plan.result.is_some() && operands.len() == plan.parameters.len().saturating_add(1);
    let parameter_count = operands
        .len()
        .saturating_sub(usize::from(has_result_operand));
    let word = ValueShape::integer(8, 8);
    omega_calling_conventions::validate_call_plan(
        plan,
        &CallSignature {
            parameters: vec![word; parameter_count],
            result: plan.result.as_ref().map(|placement| placement.shape),
        },
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "final outbound syscall replay retained an incompatible normalized plan: {error}"
        ))
    })?;
    let expected_policy = match architecture {
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
    };
    if plan.policy != expected_policy
        || plan.stack_alignment != 16
        || plan.shadow_bytes != 0
        || (architecture == Architecture::X86_64 && immediate != 0)
        || plan.parameters.len() != parameter_count
    {
        return Err(Diagnostic::error(
            "final outbound syscall replay retained incompatible policy, stack, or arity",
        ));
    }
    let fixed_scratch = match architecture {
        Architecture::X86_64 => &[MachineRegister::X86Rax, MachineRegister::X86R11][..],
        Architecture::Aarch64 => &[][..],
    };
    if fixed_scratch
        .iter()
        .copied()
        .chain([number_register])
        .any(|register| !plan.ordinary_clobbers.contains(register))
    {
        return Err(Diagnostic::error(
            "final outbound syscall replay exceeds its ordinary-clobber ceiling",
        ));
    }
    let parameter_registers = plan
        .parameters
        .iter()
        .map(|placement| match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] => Ok(*register),
            _ => Err(Diagnostic::error(
                "final outbound syscall replay requires one full-word register per parameter",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !has_result_operand {
        let address_sites = outbound_syscall_argument_storage_sites(architecture, operands)?;
        let bytes = match architecture {
            Architecture::X86_64 => omega_isa_x86_64::encode_syscall_sequence(
                operands,
                number,
                &parameter_registers,
                number_register,
                immediate,
            )?,
            Architecture::Aarch64 => {
                let operands = operands
                    .iter()
                    .map(aarch64_outbound_syscall_operand)
                    .collect::<Result<Vec<_>, _>>()?;
                omega_isa_aarch64::encode_syscall_sequence(
                    &operands,
                    number,
                    &parameter_registers,
                    number_register,
                    immediate,
                )?
            }
        };
        return Ok((bytes, address_sites));
    }

    let Some((result, arguments)) = operands.split_first() else {
        return Err(Diagnostic::error(
            "final result-bearing outbound syscall has no result operand",
        ));
    };
    let Some((result_region, _, _)) = result.runtime_scalar_integer() else {
        return Err(Diagnostic::error(
            "final result-bearing outbound syscall has no scalar result storage",
        ));
    };
    let result_register = match plan
        .result
        .as_ref()
        .map(|placement| placement.locations.as_slice())
    {
        Some(
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ],
        ) => *register,
        _ => {
            return Err(Diagnostic::error(
                "final outbound syscall result requires one full-word register",
            ));
        }
    };
    let (bytes, result_site) = match architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_value_syscall_sequence(
            operands,
            number,
            &parameter_registers,
            result_register,
            number_register,
            immediate,
        )?,
        Architecture::Aarch64 => {
            let operands = operands
                .iter()
                .map(aarch64_outbound_syscall_operand)
                .collect::<Result<Vec<_>, _>>()?;
            omega_isa_aarch64::encode_value_syscall_sequence(
                &operands,
                number,
                &parameter_registers,
                result_register,
                number_register,
                immediate,
            )?
        }
    };
    let address_site = match architecture {
        Architecture::X86_64 => result_site.checked_sub(2).ok_or_else(|| {
            Diagnostic::error("x86 outbound syscall result relocation precedes its address opcode")
        })?,
        Architecture::Aarch64 => result_site,
    };
    let mut address_sites = outbound_syscall_argument_storage_sites(architecture, arguments)?;
    address_sites.push((address_site, result_region));
    Ok((bytes, address_sites))
}
