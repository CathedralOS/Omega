use super::*;

pub(super) fn decode_instruction(
    cursor: &mut Cursor<'_>,
    allow_i64_less_than: bool,
    allow_scalar_call: bool,
    allow_jump: bool,
) -> Result<InstructionMachineEffects, PreAllocationMachineEffectDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let kind = decode_kind(cursor, allow_i64_less_than, allow_scalar_call, allow_jump)?;
    let constraint = decode_constraint_key(cursor)?;
    let unit_uses = decode_units(cursor)?;
    let unit_defs = decode_units(cursor)?;
    let unit_clobbers = decode_units(cursor)?;
    let memory = match cursor.byte()? {
        0 => MachineMemoryEffect::NoneV1,
        1 => MachineMemoryEffect::ReadPointerV1,
        5 => MachineMemoryEffect::HostedReadByteV1,
        3 => MachineMemoryEffect::HostedWriteByteV1,
        2 => MachineMemoryEffect::WriteFrameStorageV1,
        4 => MachineMemoryEffect::WritePointerV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        3 => MachineTrapBehavior::HostedExitReturnedV1,
        4 => MachineTrapBehavior::HostedReadFailureV1,
        2 => MachineTrapBehavior::HostedWriteFailureV1,
        0 => MachineTrapBehavior::NeverV1,
        1 if allow_scalar_call => MachineTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let barrier = match cursor.byte()? {
        3 => MachineBarrier::ExternalEffect,
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
            allow_jump,
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
    allow_jump: bool,
) -> Result<SelectedInstructionKind, PreAllocationMachineEffectDecodeError> {
    Ok(match cursor.byte()? {
        24 => {
            let byte_offset = cursor.u32()?;
            let byte_size = cursor.byte()?;
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(PreAllocationMachineEffectDecodeError::InvalidField);
            }
            SelectedInstructionKind::Store {
                byte_offset,
                byte_size,
            }
        }
        25 => SelectedInstructionKind::AddressOffset {
            byte_offset: cursor.u32()?,
        },
        0 => SelectedInstructionKind::CompareI64Zero,
        1 => SelectedInstructionKind::MaterializeI64 {
            value: decode_integer(cursor)?,
        },
        2 => SelectedInstructionKind::CopyI64,
        26 => SelectedInstructionKind::Float32ToBits,
        27 => SelectedInstructionKind::Float64ToBits,
        28 => SelectedInstructionKind::BitsToFloat32,
        29 => SelectedInstructionKind::BitsToFloat64,
        15 => SelectedInstructionKind::ZeroExtendU8,
        20 => SelectedInstructionKind::ZeroExtendU32,
        37 => SelectedInstructionKind::ZeroExtendU16,
        38 => SelectedInstructionKind::SignExtendI8,
        39 => SelectedInstructionKind::SignExtendI16,
        40 => SelectedInstructionKind::SignExtendI32,
        41 => SelectedInstructionKind::MaterializeBooleanEqual,
        42 => SelectedInstructionKind::MaterializeBooleanU64LessThan,
        43 => SelectedInstructionKind::MaterializeBooleanI64LessThan,
        44 => SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
        45 => SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
        32 => SelectedInstructionKind::HostedReadByte {
            slot: decode_local_storage_slot(cursor)?,
        },
        23 => SelectedInstructionKind::HostedWriteByteI32 {
            slot: decode_local_storage_slot(cursor)?,
        },
        22 => SelectedInstructionKind::ByteViewAddress,
        21 => SelectedInstructionKind::Load8Indexed,
        16 => SelectedInstructionKind::Load64 {
            byte_offset: cursor.u32()?,
        },
        tag @ (46 | 47) => {
            let byte_offset = cursor.u32()?;
            let width = crate::PackedByteWidth::from_byte_size(cursor.byte()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
            if tag == 46 {
                SelectedInstructionKind::LoadPacked { byte_offset, width }
            } else {
                SelectedInstructionKind::StorePacked { byte_offset, width }
            }
        }
        33 => SelectedInstructionKind::Load8 {
            byte_offset: cursor.u32()?,
        },
        34 => SelectedInstructionKind::Load16 {
            byte_offset: cursor.u32()?,
        },
        30 => SelectedInstructionKind::Load32 {
            byte_offset: cursor.u32()?,
        },
        tag @ (17 | 18) => {
            let slot = match cursor.byte()? {
                2 => crate::FrameStorageSlotId::Incoming {
                    parameter_index: cursor.u32()?,
                    abi_stack_byte_offset: cursor.u32()?,
                },
                slot_tag @ (0 | 3) => {
                    crate::FrameStorageSlotId::Outgoing(crate::OutgoingArgumentSlotId {
                        role: if slot_tag == 0 {
                            crate::OutgoingArgumentSlotRole::Argument
                        } else {
                            crate::OutgoingArgumentSlotRole::ValueCopy
                        },
                        operation: OperationId::new(cursor.u64()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                        argument_index: cursor.u32()?,
                    })
                }
                1 => crate::FrameStorageSlotId::Local(decode_local_storage_slot(cursor)?),
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let byte_offset = cursor.u32()?;
            if tag == 17 {
                SelectedInstructionKind::Store64 { slot, byte_offset }
            } else {
                SelectedInstructionKind::FrameAddress { slot, byte_offset }
            }
        }
        19 => SelectedInstructionKind::CallUnit {
            callee: MachineId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
        },
        51 => SelectedInstructionKind::BitwiseAndI64,
        52 => SelectedInstructionKind::BitwiseXorI64,
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
        31 => SelectedInstructionKind::HostedExitProcessI32,
        7 => SelectedInstructionKind::ReturnScalar,
        36 => SelectedInstructionKind::ReturnAggregate {
            fragment_count: cursor.byte()?,
        },
        35 if allow_scalar_call => SelectedInstructionKind::CallAggregate {
            callee: MachineId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
        },
        8 => SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        9 => SelectedInstructionKind::ReturnUnit,
        10 => SelectedInstructionKind::CompareI64,
        53 => SelectedInstructionKind::CompareI64Immediate {
            immediate: decode_integer(cursor)?,
        },
        11 => SelectedInstructionKind::ConditionalBranchU64LessThan,
        12 if allow_i64_less_than => SelectedInstructionKind::ConditionalBranchI64LessThan,
        14 if allow_jump => SelectedInstructionKind::Jump,
        13 if allow_scalar_call => SelectedInstructionKind::CallScalar {
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

pub fn decode_local_storage_slot(
    cursor: &mut Cursor<'_>,
) -> Result<crate::LocalStorageSlotId, PreAllocationMachineEffectDecodeError> {
    let tag = cursor.byte()?;
    if tag == 4 {
        return Ok(crate::LocalStorageSlotId::StructuralParameter {
            place: semantic_vocabulary::PlaceId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
        });
    }
    if tag == 3 {
        return Ok(crate::LocalStorageSlotId::StructuralBlockParameter {
            block: semantic_vocabulary::BlockId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
            place: semantic_vocabulary::PlaceId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
        });
    }
    if tag == 2 {
        return Ok(crate::LocalStorageSlotId::Spill {
            register: crate::VirtualRegisterId(cursor.u32()?),
        });
    }
    let operation = OperationId::new(cursor.u64()?)
        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
    match tag {
        0 => Ok(crate::LocalStorageSlotId::Structural {
            operation,
            place: semantic_vocabulary::PlaceId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
        }),
        1 => Ok(crate::LocalStorageSlotId::Boundary { operation }),
        _ => Err(PreAllocationMachineEffectDecodeError::InvalidField),
    }
}

pub fn decode_provenance(
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

pub fn decode_alternative(
    cursor: &mut Cursor<'_>,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    decode_alternative_for_version(cursor, true, true, true)
}

pub fn decode_alternative_legacy(
    cursor: &mut Cursor<'_>,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    decode_alternative_for_version(cursor, false, false, false)
}

pub fn decode_alternative_without_jump(
    cursor: &mut Cursor<'_>,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    decode_alternative_for_version(cursor, true, true, false)
}

pub fn decode_alternative_without_scalar_call(
    cursor: &mut Cursor<'_>,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    decode_alternative_for_version(cursor, true, false, false)
}

fn decode_alternative_for_version(
    cursor: &mut Cursor<'_>,
    allow_i64_less_than: bool,
    allow_scalar_call: bool,
    allow_jump: bool,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    let family = match cursor.byte()? {
        0 => MachineAlternativeFamily::CompareI64Zero,
        1 => MachineAlternativeFamily::MaterializeI64,
        2 => MachineAlternativeFamily::CopyI64,
        26 => MachineAlternativeFamily::Float32ToBits,
        27 => MachineAlternativeFamily::Float64ToBits,
        28 => MachineAlternativeFamily::BitsToFloat32,
        29 => MachineAlternativeFamily::BitsToFloat64,
        15 => MachineAlternativeFamily::ZeroExtendU8,
        20 => MachineAlternativeFamily::ZeroExtendU32,
        37 => MachineAlternativeFamily::ZeroExtendU16,
        38 => MachineAlternativeFamily::SignExtendI8,
        39 => MachineAlternativeFamily::SignExtendI16,
        40 => MachineAlternativeFamily::SignExtendI32,
        41 => MachineAlternativeFamily::MaterializeBooleanEqual,
        42 => MachineAlternativeFamily::MaterializeBooleanU64LessThan,
        43 => MachineAlternativeFamily::MaterializeBooleanI64LessThan,
        44 => MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual,
        45 => MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual,

        31 => MachineAlternativeFamily::HostedExitProcessI32,
        32 => MachineAlternativeFamily::HostedReadByte,
        23 => MachineAlternativeFamily::HostedWriteByteI32,
        22 => MachineAlternativeFamily::ByteViewAddress,
        21 => MachineAlternativeFamily::Load8Indexed,
        16 => MachineAlternativeFamily::Load64,
        46 => MachineAlternativeFamily::LoadPacked3,
        47 => MachineAlternativeFamily::LoadPacked5,
        48 => MachineAlternativeFamily::LoadPacked6,
        49 => MachineAlternativeFamily::LoadPacked7,
        50 => MachineAlternativeFamily::StorePacked,
        33 => MachineAlternativeFamily::Load8,
        34 => MachineAlternativeFamily::Load16,
        30 => MachineAlternativeFamily::Load32,
        24 => MachineAlternativeFamily::Store,
        25 => MachineAlternativeFamily::AddressOffset,
        17 => MachineAlternativeFamily::Store64,
        18 => MachineAlternativeFamily::FrameAddress,
        19 => MachineAlternativeFamily::CallUnit,
        3 => MachineAlternativeFamily::ExactAddI64,
        51 => MachineAlternativeFamily::BitwiseAndI64,
        52 => MachineAlternativeFamily::BitwiseXorI64,
        4 => MachineAlternativeFamily::ExactAddI64Immediate,
        5 => MachineAlternativeFamily::ExactSubtractI64,
        6 => MachineAlternativeFamily::ConditionalBranchNonZero,
        7 => MachineAlternativeFamily::ReturnScalar,
        35 if allow_scalar_call => MachineAlternativeFamily::CallAggregate,
        36 => MachineAlternativeFamily::ReturnAggregate,
        8 => MachineAlternativeFamily::ExactSubtractI64Immediate,
        9 => MachineAlternativeFamily::ReturnUnit,
        10 => MachineAlternativeFamily::CompareI64,
        53 => MachineAlternativeFamily::CompareI64Immediate,
        11 => MachineAlternativeFamily::ConditionalBranchU64LessThan,
        12 if allow_i64_less_than => MachineAlternativeFamily::ConditionalBranchI64LessThan,
        13 if allow_scalar_call => MachineAlternativeFamily::CallScalar,
        14 if allow_jump => MachineAlternativeFamily::Jump,
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
            excluded_view: register_model::RegisterViewId(cursor.u16()?),
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
    let encoded = decode_encoded_effects(cursor, allow_scalar_call, allow_jump)?;
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
    allow_jump: bool,
) -> Result<MachineEncodedEffects, PreAllocationMachineEffectDecodeError> {
    let external_operand_reads = decode_u16s(cursor)?;
    let external_operand_writes = decode_u16s(cursor)?;
    let implicit_unit_uses = decode_units(cursor)?;
    let implicit_unit_defs = decode_units(cursor)?;
    let implicit_unit_clobbers = decode_units(cursor)?;
    let memory = match cursor.byte()? {
        8 => MachineEncodedMemoryEffect::HostedReadByteV1 {
            stack_pointer: register_model::RegisterViewId(cursor.u16()?),
        },
        6 => MachineEncodedMemoryEffect::HostedWriteByteV1 {
            stack_pointer: register_model::RegisterViewId(cursor.u16()?),
        },
        0 => MachineEncodedMemoryEffect::NoneV1,
        7 => MachineEncodedMemoryEffect::WritePointerV1 {
            pointer_operand: cursor.u16()?,
        },
        5 => MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
            pointer_operand: cursor.u16()?,
            index_operand: cursor.u16()?,
            byte_count: cursor.u16()?,
        },
        3 => MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: cursor.u16()?,
            byte_count: cursor.u16()?,
        },
        4 => MachineEncodedMemoryEffect::WriteFrameStorageV1 {
            stack_pointer: register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        1 => MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer: register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        2 if allow_scalar_call => {
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                byte_count: cursor.u16()?,
            }
        }
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let stack = match cursor.byte()? {
        0 => MachineEncodedStackEffect::UnchangedV1,
        1 => MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer: register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        2 if allow_scalar_call => MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer: register_model::RegisterViewId(cursor.u16()?),
            return_address_byte_count: cursor.u16()?,
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        3 => MachineEncodedTrapBehavior::HostedExitReturnedV1,
        4 => MachineEncodedTrapBehavior::HostedReadFailureV1,
        2 => MachineEncodedTrapBehavior::HostedWriteFailureV1,
        0 => MachineEncodedTrapBehavior::NeverV1,
        1 => MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let control = match cursor.byte()? {
        7 => MachineEncodedControlEffect::HostedExitOrTrapV1,
        8 => MachineEncodedControlEffect::HostedReadReturnOrTrapV1,
        6 => MachineEncodedControlEffect::HostedWriteReturnOrTrapV1,
        0 => MachineEncodedControlEffect::FallThroughV1,
        1 => MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        2 => MachineEncodedControlEffect::ReturnFromActivationStackV1,
        3 => MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
            target: register_model::RegisterViewId(cursor.u16()?),
        },
        4 if allow_scalar_call => MachineEncodedControlEffect::DirectRelativeCallV1,
        5 if allow_jump => MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
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

#[cfg(test)]
mod local_slot_tests {
    use super::*;
    use crate::LocalStorageSlotId;
    use semantic_vocabulary::{BlockId, PlaceId};

    #[test]
    fn owned_entry_slot_codec_retains_place_without_operation_or_block_identity() {
        let place = PlaceId::new(0x0102_0304_0506_0708).unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place };
        assert_eq!(slot.operation(), None);
        assert_eq!(slot.structural_place(), Some(place));
        let mut encoded = Vec::new();
        slot.encode_identity(&mut encoded);
        assert_eq!(encoded, [4, 8, 7, 6, 5, 4, 3, 2, 1]);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_local_storage_slot(&mut cursor).unwrap(), slot);
        assert_eq!(cursor.remaining(), 0);

        for changed in [
            LocalStorageSlotId::StructuralParameter {
                place: PlaceId::new(place.get() + 1).unwrap(),
            },
            LocalStorageSlotId::StructuralBlockParameter {
                block: BlockId::new(1).unwrap(),
                place,
            },
            LocalStorageSlotId::Structural {
                operation: OperationId::new(1).unwrap(),
                place,
            },
        ] {
            let mut changed_bytes = Vec::new();
            changed.encode_identity(&mut changed_bytes);
            assert_ne!(changed_bytes, encoded);
        }
    }

    #[test]
    fn owned_entry_slot_decoder_rejects_zero_truncation_and_unknown_tag() {
        let encoded = [4, 1, 0, 0, 0, 0, 0, 0, 0];
        for length in 0..encoded.len() {
            assert!(decode_local_storage_slot(&mut Cursor::new(&encoded[..length])).is_err());
        }
        let mut zero_place = encoded;
        zero_place[1] = 0;
        assert_eq!(
            decode_local_storage_slot(&mut Cursor::new(&zero_place)),
            Err(PreAllocationMachineEffectDecodeError::InvalidField)
        );
        let mut unknown_tag = encoded;
        unknown_tag[0] = 5;
        assert_eq!(
            decode_local_storage_slot(&mut Cursor::new(&unknown_tag)),
            Err(PreAllocationMachineEffectDecodeError::InvalidField)
        );
    }
}

#[cfg(test)]
mod packed_tests {
    use super::*;
    #[test]
    fn packed_instruction_decode_retains_width_and_rejects_invalid_footprints() {
        for tag in [46, 47] {
            for raw in 0..=8 {
                let mut bytes = vec![tag];
                bytes.extend_from_slice(&37_u32.to_le_bytes());
                bytes.push(raw);
                let decoded = decode_kind(&mut Cursor::new(&bytes), true, true, true);
                if let Some(width) = crate::PackedByteWidth::from_byte_size(raw) {
                    let expected = if tag == 46 {
                        SelectedInstructionKind::LoadPacked {
                            byte_offset: 37,
                            width,
                        }
                    } else {
                        SelectedInstructionKind::StorePacked {
                            byte_offset: 37,
                            width,
                        }
                    };
                    assert_eq!(decoded.unwrap(), expected);
                } else {
                    assert!(decoded.is_err());
                }
                for length in 0..bytes.len() {
                    assert!(
                        decode_kind(&mut Cursor::new(&bytes[..length]), true, true, true).is_err()
                    );
                }
            }
        }
    }
}
