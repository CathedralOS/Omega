//! Replays table- and vtable-field indirect calls.

use super::*;

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

pub(in crate::checked) fn encode_aarch64_indirect_call_replay(
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

pub(in crate::checked) fn encode_indirect_call_replay(
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
