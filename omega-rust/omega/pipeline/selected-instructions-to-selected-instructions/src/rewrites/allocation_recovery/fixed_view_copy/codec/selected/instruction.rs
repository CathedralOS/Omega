use register_model::{RegisterClassId, RegisterUnitId, RegisterViewId};
use selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedOperand,
    VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, ObligationId, OperationId};

use crate::FixedViewCopyDecodeError;

use super::provenance::{decode_provenance, encode_provenance};
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::{
    primitives::{
        Cursor, decode_id, decode_option_u16, decode_u16s, encode_option_u16, encode_u16s, length,
    },
    values::{
        access_tag, decode_access, decode_bool, decode_constraint_key, decode_integer,
        encode_constraint_key, encode_integer,
    },
};

pub(super) fn encode_instruction(bytes: &mut Vec<u8>, instruction: &SelectedInstruction) {
    bytes.extend_from_slice(&instruction.id.0.to_le_bytes());
    encode_kind(bytes, instruction.kind);
    encode_constraint_key(bytes, instruction.constraint);
    length(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        bytes.extend_from_slice(&operand.operand.to_le_bytes());
        bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
        bytes.push(access_tag(operand.access));
        bytes.extend_from_slice(&operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_u16s(bytes, instruction.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.clobbers.iter().map(|unit| unit.0));
    encode_provenance(bytes, &instruction.provenance);
}

pub(super) fn decode_instruction(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstruction, FixedViewCopyDecodeError> {
    let id = SelectedInstructionId(cursor.u32()?);
    let kind = decode_kind(cursor)?;
    let constraint = decode_constraint_key(cursor)?;
    let operand_count = cursor.length()?;
    let mut operands = Vec::with_capacity(operand_count.min(cursor.remaining()));
    for _ in 0..operand_count {
        operands.push(SelectedOperand {
            operand: cursor.u16()?,
            virtual_register: VirtualRegisterId(cursor.u32()?),
            access: decode_access(cursor)?,
            class: RegisterClassId(cursor.u16()?),
            fixed_view: decode_option_u16(cursor)?.map(RegisterViewId),
            tied_to: decode_option_u16(cursor)?,
            early_clobber: decode_bool(cursor)?,
        });
    }
    Ok(SelectedInstruction {
        id,
        kind,
        constraint,
        operands,
        implicit_uses: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        implicit_defs: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        clobbers: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        provenance: decode_provenance(cursor)?,
    })
}

fn encode_kind(bytes: &mut Vec<u8>, kind: SelectedInstructionKind) {
    let tag = match kind {
        SelectedInstructionKind::CallAggregate { .. } => 35,
        SelectedInstructionKind::ReturnAggregate { .. } => 36,
        SelectedInstructionKind::Store { .. } => 24,
        SelectedInstructionKind::AddressOffset { .. } => 25,
        SelectedInstructionKind::Load64 { .. } => 16,
        SelectedInstructionKind::LoadPacked { .. } => 46,
        SelectedInstructionKind::StorePacked { .. } => 47,
        SelectedInstructionKind::Load8 { .. } => 33,
        SelectedInstructionKind::Load16 { .. } => 34,
        SelectedInstructionKind::Load32 { .. } => 30,
        SelectedInstructionKind::HostedWriteByteI32 { .. } => 23,
        SelectedInstructionKind::HostedReadByte { .. } => 32,
        SelectedInstructionKind::HostedExitProcessI32 => 31,
        SelectedInstructionKind::ByteViewAddress => 22,
        SelectedInstructionKind::Load8Indexed => 21,
        SelectedInstructionKind::Store64 { .. } => 17,
        SelectedInstructionKind::FrameAddress { .. } => 18,
        SelectedInstructionKind::CallUnit { .. } => 19,
        SelectedInstructionKind::Jump => 14,
        SelectedInstructionKind::CompareI64Zero => 0,
        SelectedInstructionKind::MaterializeI64 { .. } => 1,
        SelectedInstructionKind::ConditionalBranchNonZero => 2,
        SelectedInstructionKind::ReturnScalar => 3,
        SelectedInstructionKind::CopyI64 => 4,
        SelectedInstructionKind::BitwiseAndI64 => 51,
        SelectedInstructionKind::Float32ToBits => 26,
        SelectedInstructionKind::Float64ToBits => 27,
        SelectedInstructionKind::BitsToFloat32 => 28,
        SelectedInstructionKind::BitsToFloat64 => 29,
        SelectedInstructionKind::ZeroExtendU8 => 15,
        SelectedInstructionKind::ZeroExtendU32 => 20,
        SelectedInstructionKind::ZeroExtendU16 => 37,
        SelectedInstructionKind::SignExtendI8 => 38,
        SelectedInstructionKind::SignExtendI16 => 39,
        SelectedInstructionKind::SignExtendI32 => 40,
        SelectedInstructionKind::MaterializeBooleanEqual => 41,
        SelectedInstructionKind::MaterializeBooleanU64LessThan => 42,
        SelectedInstructionKind::MaterializeBooleanI64LessThan => 43,
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 44,
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 45,
        SelectedInstructionKind::ExactAddI64 { .. } => 5,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => 6,
        SelectedInstructionKind::ExactSubtractI64 { .. } => 7,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        SelectedInstructionKind::ReturnUnit => 9,
        SelectedInstructionKind::CompareI64 => 10,
        SelectedInstructionKind::ConditionalBranchU64LessThan => 11,
        SelectedInstructionKind::CallScalar { .. } => 12,
        SelectedInstructionKind::ConditionalBranchI64LessThan => 13,
    };
    bytes.push(tag);
    match kind {
        SelectedInstructionKind::LoadPacked { byte_offset, width }
        | SelectedInstructionKind::StorePacked { byte_offset, width } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
            bytes.push(width.byte_size());
        }
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
            bytes.push(byte_size);
        }
        SelectedInstructionKind::AddressOffset { byte_offset } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::Load64 { byte_offset }
        | SelectedInstructionKind::Load8 { byte_offset }
        | SelectedInstructionKind::Load16 { byte_offset }
        | SelectedInstructionKind::Load32 { byte_offset } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes())
        }
        SelectedInstructionKind::Store64 { slot, byte_offset }
        | SelectedInstructionKind::FrameAddress { slot, byte_offset } => {
            match slot {
                selected_instructions::FrameStorageSlotId::Incoming {
                    parameter_index,
                    abi_stack_byte_offset,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&parameter_index.to_le_bytes());
                    bytes.extend_from_slice(&abi_stack_byte_offset.to_le_bytes());
                }
                selected_instructions::FrameStorageSlotId::Outgoing(slot) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&slot.operation.get().to_le_bytes());
                    bytes.extend_from_slice(&slot.argument_index.to_le_bytes());
                }
                selected_instructions::FrameStorageSlotId::Local(slot) => {
                    bytes.push(1);
                    slot.encode_identity(bytes);
                }
            }
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::HostedWriteByteI32 { slot }
        | SelectedInstructionKind::HostedReadByte { slot } => slot.encode_identity(bytes),
        SelectedInstructionKind::MaterializeI64 { value } => encode_integer(bytes, value),
        SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        } => {
            encode_integer(bytes, immediate);
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::CallScalar { callee }
        | SelectedInstructionKind::CallAggregate { callee }
        | SelectedInstructionKind::CallUnit { callee } => {
            bytes.extend_from_slice(&callee.get().to_le_bytes());
        }
        SelectedInstructionKind::ReturnAggregate { fragment_count } => bytes.push(fragment_count),
        _ => {}
    }
}

#[cfg(test)]
#[test]
fn zero_extension_has_a_distinct_round_trip_tag() {
    for (kind, tag) in [
        (SelectedInstructionKind::ZeroExtendU8, 15),
        (SelectedInstructionKind::BitwiseAndI64, 51),
        (SelectedInstructionKind::ZeroExtendU32, 20),
        (SelectedInstructionKind::ZeroExtendU16, 37),
        (SelectedInstructionKind::SignExtendI8, 38),
        (SelectedInstructionKind::SignExtendI16, 39),
        (SelectedInstructionKind::SignExtendI32, 40),
        (SelectedInstructionKind::MaterializeBooleanEqual, 41),
        (SelectedInstructionKind::MaterializeBooleanU64LessThan, 42),
        (SelectedInstructionKind::MaterializeBooleanI64LessThan, 43),
        (
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
            44,
        ),
        (
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
            45,
        ),
        (SelectedInstructionKind::Load8Indexed, 21),
        (SelectedInstructionKind::ByteViewAddress, 22),
    ] {
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, kind);
        assert_eq!(bytes, [tag]);
        assert_eq!(decode_kind(&mut Cursor::new(&bytes)).unwrap(), kind);
    }
}

pub(in crate::rewrites::allocation_recovery::fixed_view_copy::codec) fn decode_kind(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionKind, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        35 => SelectedInstructionKind::CallAggregate {
            callee: decode_id(cursor, MachineId::new)?,
        },
        36 => SelectedInstructionKind::ReturnAggregate {
            fragment_count: cursor.byte()?,
        },
        24 => SelectedInstructionKind::Store {
            byte_offset: cursor.u32()?,
            byte_size: cursor.byte()?,
        },
        25 => SelectedInstructionKind::AddressOffset {
            byte_offset: cursor.u32()?,
        },
        16 => SelectedInstructionKind::Load64 {
            byte_offset: cursor.u32()?,
        },
        31 => SelectedInstructionKind::HostedExitProcessI32,
        tag @ (46 | 47) => {
            let byte_offset = cursor.u32()?;
            let raw_width = cursor.byte()?;
            let width = selected_instructions::PackedByteWidth::from_byte_size(raw_width)
                .ok_or(FixedViewCopyDecodeError::InvalidPackedByteWidth(raw_width))?;
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
                2 => selected_instructions::FrameStorageSlotId::Incoming {
                    parameter_index: cursor.u32()?,
                    abi_stack_byte_offset: cursor.u32()?,
                },
                0 => selected_instructions::FrameStorageSlotId::Outgoing(
                    selected_instructions::OutgoingArgumentSlotId {
                        operation: decode_id(cursor, OperationId::new)?,
                        argument_index: cursor.u32()?,
                    },
                ),
                1 => selected_instructions::FrameStorageSlotId::Local(
                    super::structural::decode_local_slot(cursor)?,
                ),
                tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
            };
            let byte_offset = cursor.u32()?;
            if tag == 17 {
                SelectedInstructionKind::Store64 { slot, byte_offset }
            } else {
                SelectedInstructionKind::FrameAddress { slot, byte_offset }
            }
        }
        19 => SelectedInstructionKind::CallUnit {
            callee: decode_id(cursor, MachineId::new)?,
        },
        14 => SelectedInstructionKind::Jump,
        0 => SelectedInstructionKind::CompareI64Zero,
        1 => SelectedInstructionKind::MaterializeI64 {
            value: decode_integer(cursor)?,
        },
        2 => SelectedInstructionKind::ConditionalBranchNonZero,
        3 => SelectedInstructionKind::ReturnScalar,
        4 => SelectedInstructionKind::CopyI64,
        51 => SelectedInstructionKind::BitwiseAndI64,
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
            slot: super::structural::decode_local_slot(cursor)?,
        },
        23 => SelectedInstructionKind::HostedWriteByteI32 {
            slot: super::structural::decode_local_slot(cursor)?,
        },
        22 => SelectedInstructionKind::ByteViewAddress,
        21 => SelectedInstructionKind::Load8Indexed,
        5 => SelectedInstructionKind::ExactAddI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        6 => SelectedInstructionKind::ExactAddI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        7 => SelectedInstructionKind::ExactSubtractI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        8 => SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        9 => SelectedInstructionKind::ReturnUnit,
        10 => SelectedInstructionKind::CompareI64,
        11 => SelectedInstructionKind::ConditionalBranchU64LessThan,
        12 => SelectedInstructionKind::CallScalar {
            callee: decode_id(cursor, MachineId::new)?,
        },
        13 => SelectedInstructionKind::ConditionalBranchI64LessThan,
        tag => return Err(FixedViewCopyDecodeError::UnknownInstructionKind(tag)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_i64_uses_append_only_tag_ten() {
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, SelectedInstructionKind::CompareI64);
        assert_eq!(bytes, [10]);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            decode_kind(&mut cursor).unwrap(),
            SelectedInstructionKind::CompareI64
        );
    }

    #[test]
    fn conditional_branch_u64_less_than_uses_append_only_tag_eleven() {
        let mut bytes = Vec::new();
        encode_kind(
            &mut bytes,
            SelectedInstructionKind::ConditionalBranchU64LessThan,
        );
        assert_eq!(bytes, [11]);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            decode_kind(&mut cursor).unwrap(),
            SelectedInstructionKind::ConditionalBranchU64LessThan
        );
    }

    #[test]
    fn call_i64_uses_append_only_tag_twelve_and_binds_callee() {
        let callee = MachineId::new(47).unwrap();
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, SelectedInstructionKind::CallScalar { callee });
        assert_eq!(bytes[0], 12);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            decode_kind(&mut cursor).unwrap(),
            SelectedInstructionKind::CallScalar { callee }
        );
    }

    #[test]
    fn conditional_branch_i64_less_than_uses_append_only_tag_thirteen() {
        let mut bytes = Vec::new();
        encode_kind(
            &mut bytes,
            SelectedInstructionKind::ConditionalBranchI64LessThan,
        );
        assert_eq!(bytes, [13]);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            decode_kind(&mut cursor).unwrap(),
            SelectedInstructionKind::ConditionalBranchI64LessThan
        );
    }
}
#[test]
#[cfg(test)]
fn structural_primitives_round_trip_symbolic_slots_without_scalar_results() {
    let slot = selected_instructions::OutgoingArgumentSlotId {
        operation: semantic_vocabulary::OperationId::new(43).unwrap(),
        argument_index: 1,
    };
    for kind in [
        SelectedInstructionKind::HostedReadByte {
            slot: selected_instructions::LocalStorageSlotId::Structural {
                operation: slot.operation,
                place: semantic_vocabulary::PlaceId::new(47).unwrap(),
            },
        },
        SelectedInstructionKind::FrameAddress {
            slot: selected_instructions::FrameStorageSlotId::Incoming {
                parameter_index: 8,
                abi_stack_byte_offset: 16,
            },
            byte_offset: 0,
        },
        SelectedInstructionKind::HostedWriteByteI32 {
            slot: selected_instructions::LocalStorageSlotId::Boundary {
                operation: slot.operation,
            },
        },
        SelectedInstructionKind::Store {
            byte_offset: 13,
            byte_size: 1,
        },
        SelectedInstructionKind::Store {
            byte_offset: 26,
            byte_size: 2,
        },
        SelectedInstructionKind::Store {
            byte_offset: 52,
            byte_size: 4,
        },
        SelectedInstructionKind::Store {
            byte_offset: 104,
            byte_size: 8,
        },
        SelectedInstructionKind::AddressOffset { byte_offset: 26 },
        SelectedInstructionKind::Load64 { byte_offset: 8 },
        SelectedInstructionKind::Load8 { byte_offset: 3 },
        SelectedInstructionKind::Load16 { byte_offset: 6 },
        SelectedInstructionKind::Load32 { byte_offset: 12 },
        SelectedInstructionKind::Float32ToBits,
        SelectedInstructionKind::Float64ToBits,
        SelectedInstructionKind::BitsToFloat32,
        SelectedInstructionKind::BitsToFloat64,
        SelectedInstructionKind::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
            byte_offset: 8,
        },
        SelectedInstructionKind::FrameAddress {
            slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
            byte_offset: 0,
        },
        SelectedInstructionKind::CallUnit {
            callee: MachineId::new(47).unwrap(),
        },
    ] {
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, kind);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_kind(&mut cursor).unwrap(), kind);
        assert_eq!(cursor.remaining(), 0);
    }
}
