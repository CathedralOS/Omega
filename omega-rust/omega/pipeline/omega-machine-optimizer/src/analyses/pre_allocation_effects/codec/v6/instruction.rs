use super::*;

pub(super) fn decode_instruction(
    cursor: &mut Cursor<'_>,
    allow_i64_less_than: bool,
    allow_scalar_call: bool,
) -> Result<InstructionMachineEffects, PreAllocationMachineEffectDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let kind = decode_kind(cursor, allow_i64_less_than, allow_scalar_call)?;
    let constraint = decode_constraint_key(cursor)?;
    let unit_uses = decode_units(cursor)?;
    let unit_defs = decode_units(cursor)?;
    let unit_clobbers = decode_units(cursor)?;
    let memory = match cursor.byte()? {
        0 => MachineMemoryEffect::NoneV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        0 => MachineTrapBehavior::NeverV1,
        1 if allow_scalar_call => MachineTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let barrier = match cursor.byte()? {
        0 => MachineBarrier::None,
        1 => MachineBarrier::ControlFlow,
        2 if allow_scalar_call => MachineBarrier::Call,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let call = match cursor.byte()? {
        0 => MachineCallEffect::NoneV1,
        1 if allow_scalar_call => MachineCallEffect::DirectInternalNormalReturnV1 {
            pre_call_stack_alignment: cursor.u16()?,
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 0 {
        return Err(PreAllocationMachineEffectDecodeError::InvalidField);
    }
    let provenance = decode_provenance(cursor)?;
    let alternative_count = cursor.length()?;
    let mut alternatives = Vec::with_capacity(alternative_count.min(cursor.remaining()));
    for _ in 0..alternative_count {
        alternatives.push(decode_alternative_for_version(
            cursor,
            allow_i64_less_than,
            allow_scalar_call,
        )?);
    }
    Ok(InstructionMachineEffects {
        instruction,
        kind,
        constraint,
        unit_uses,
        unit_defs,
        unit_clobbers,
        memory,
        trap,
        barrier,
        call,
        cleanup: MachineCleanupEffect::NoneV1,
        provenance,
        alternatives,
    })
}

fn decode_kind(
    cursor: &mut Cursor<'_>,
    allow_i64_less_than: bool,
    allow_scalar_call: bool,
) -> Result<SelectedInstructionKind, PreAllocationMachineEffectDecodeError> {
    Ok(match cursor.byte()? {
        0 => SelectedInstructionKind::CompareI64Zero,
        1 => SelectedInstructionKind::MaterializeI64 {
            value: decode_integer(cursor)?,
        },
        2 => SelectedInstructionKind::CopyI64,
        3 => SelectedInstructionKind::ExactAddI64 {
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        4 => SelectedInstructionKind::ExactAddI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        5 => SelectedInstructionKind::ExactSubtractI64 {
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        6 => SelectedInstructionKind::ConditionalBranchNonZero,
        7 => SelectedInstructionKind::ReturnI64,
        8 => SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        9 => SelectedInstructionKind::ReturnUnit,
        10 => SelectedInstructionKind::CompareI64,
        11 => SelectedInstructionKind::ConditionalBranchU64LessThan,
        12 if allow_i64_less_than => SelectedInstructionKind::ConditionalBranchI64LessThan,
        13 if allow_scalar_call => SelectedInstructionKind::CallI64 {
            callee: MachineId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    })
}

fn decode_integer(
    cursor: &mut Cursor<'_>,
) -> Result<IntegerValue, PreAllocationMachineEffectDecodeError> {
    match cursor.byte()? {
        0 => Ok(IntegerValue::Signed(i128::from_le_bytes(cursor.array()?))),
        1 => Ok(IntegerValue::Unsigned(u128::from_le_bytes(cursor.array()?))),
        _ => Err(PreAllocationMachineEffectDecodeError::InvalidField),
    }
}

pub(crate) fn decode_provenance(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionProvenance, PreAllocationMachineEffectDecodeError> {
    let operations = decode_ids(cursor, OperationId::new)?;
    let values = decode_ids(cursor, ValueId::new)?;
    let edges = decode_ids(cursor, EdgeId::new)?;
    let obligations = decode_ids(cursor, ObligationId::new)?;
    let fuel_count = cursor.length()?;
    let mut fuel = Vec::with_capacity(fuel_count.min(cursor.remaining()));
    for _ in 0..fuel_count {
        let site_tag = cursor.byte()?;
        let raw = cursor.u64()?;
        let site = match site_tag {
            0 => PsiProvenance::Operation(
                OperationId::new(raw).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
            ),
            1 => PsiProvenance::Edge(
                EdgeId::new(raw).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
            ),
            _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
        };
        fuel.push(FuelSettlement {
            site,
            units: cursor.u64()?,
        });
    }
    Ok(SelectedInstructionProvenance {
        operations,
        values,
        edges,
        obligations,
        fuel,
    })
}

pub(crate) fn decode_alternative(
    cursor: &mut Cursor<'_>,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    decode_alternative_for_version(cursor, true, true)
}

pub(crate) fn decode_alternative_legacy(
    cursor: &mut Cursor<'_>,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    decode_alternative_for_version(cursor, false, false)
}

pub(crate) fn decode_alternative_without_scalar_call(
    cursor: &mut Cursor<'_>,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    decode_alternative_for_version(cursor, true, false)
}

fn decode_alternative_for_version(
    cursor: &mut Cursor<'_>,
    allow_i64_less_than: bool,
    allow_scalar_call: bool,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    let family = match cursor.byte()? {
        0 => MachineAlternativeFamily::CompareI64Zero,
        1 => MachineAlternativeFamily::MaterializeI64,
        2 => MachineAlternativeFamily::CopyI64,
        3 => MachineAlternativeFamily::ExactAddI64,
        4 => MachineAlternativeFamily::ExactAddI64Immediate,
        5 => MachineAlternativeFamily::ExactSubtractI64,
        6 => MachineAlternativeFamily::ConditionalBranchNonZero,
        7 => MachineAlternativeFamily::ReturnI64,
        8 => MachineAlternativeFamily::ExactSubtractI64Immediate,
        9 => MachineAlternativeFamily::ReturnUnit,
        10 => MachineAlternativeFamily::CompareI64,
        11 => MachineAlternativeFamily::ConditionalBranchU64LessThan,
        12 if allow_i64_less_than => MachineAlternativeFamily::ConditionalBranchI64LessThan,
        13 if allow_scalar_call => MachineAlternativeFamily::CallI64,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let key = MachineAlternativeKey {
        family,
        variant: cursor.u32()?,
    };
    let applicability = match cursor.byte()? {
        0 => MachineAlternativeApplicability::Always,
        1 => MachineAlternativeApplicability::ResultAliasesOperand {
            result: cursor.u16()?,
            operand: cursor.u16()?,
        },
        2 => MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result: cursor.u16()?,
            aliased_operand: cursor.u16()?,
            distinct_operand: cursor.u16()?,
        },
        3 => MachineAlternativeApplicability::ResultAliasesOperands {
            result: cursor.u16()?,
            left: cursor.u16()?,
            right: cursor.u16()?,
        },
        4 => MachineAlternativeApplicability::ResultDistinctFromOperands {
            result: cursor.u16()?,
            left: cursor.u16()?,
            right: cursor.u16()?,
        },
        5 => MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left: cursor.u16()?,
            right: cursor.u16()?,
            excluded_view: omega_register_model::RegisterViewId(cursor.u16()?),
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let size = match cursor.byte()? {
        0 => MachineSizeKnowledge::ExactBytes(cursor.u16()?),
        1 => {
            let minimum_bytes = cursor.u16()?;
            let maximum_bytes = match cursor.byte()? {
                0 => None,
                1 => Some(cursor.u16()?),
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes,
                maximum_bytes,
            }
        }
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 0 {
        return Err(PreAllocationMachineEffectDecodeError::InvalidField);
    }
    let encoded = decode_encoded_effects(cursor, allow_scalar_call)?;
    Ok(MachineAlternative {
        key,
        applicability,
        size,
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded,
    })
}

fn decode_encoded_effects(
    cursor: &mut Cursor<'_>,
    allow_scalar_call: bool,
) -> Result<MachineEncodedEffects, PreAllocationMachineEffectDecodeError> {
    let external_operand_reads = decode_u16s(cursor)?;
    let external_operand_writes = decode_u16s(cursor)?;
    let implicit_unit_uses = decode_units(cursor)?;
    let implicit_unit_defs = decode_units(cursor)?;
    let implicit_unit_clobbers = decode_units(cursor)?;
    let memory = match cursor.byte()? {
        0 => MachineEncodedMemoryEffect::NoneV1,
        1 => MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        2 if allow_scalar_call => {
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
                byte_count: cursor.u16()?,
            }
        }
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let stack = match cursor.byte()? {
        0 => MachineEncodedStackEffect::UnchangedV1,
        1 => MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        2 if allow_scalar_call => MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
            return_address_byte_count: cursor.u16()?,
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        0 => MachineEncodedTrapBehavior::NeverV1,
        1 => MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let control = match cursor.byte()? {
        0 => MachineEncodedControlEffect::FallThroughV1,
        1 => MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        2 => MachineEncodedControlEffect::ReturnFromActivationStackV1,
        3 => MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
            target: omega_register_model::RegisterViewId(cursor.u16()?),
        },
        4 if allow_scalar_call => MachineEncodedControlEffect::DirectRelativeCallV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    Ok(MachineEncodedEffects {
        external_operand_reads,
        external_operand_writes,
        implicit_unit_uses,
        implicit_unit_defs,
        implicit_unit_clobbers,
        memory,
        stack,
        trap,
        control,
    })
}

fn decode_u16s(cursor: &mut Cursor<'_>) -> Result<Vec<u16>, PreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(cursor.u16()?);
    }
    Ok(values)
}
