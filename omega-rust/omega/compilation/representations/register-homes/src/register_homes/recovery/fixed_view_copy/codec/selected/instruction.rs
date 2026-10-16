use legalized_operations::encode_integer;
use register_model::encode_constraint_key;
use register_model::{RegisterClassId, RegisterUnitId, RegisterViewId};
use selected_instructions::effects::program::encoding::saturating_kind;
use selected_instructions::{
    SaturatingOperation, TrappingForm, saturating_family_tag, trapping_family_tag,
};
use selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedOperand,
    VirtualRegisterId,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, ObligationId, OperationId};

use crate::FixedViewCopyDecodeError;

use super::provenance::{decode_provenance, encode_provenance};
use crate::register_homes::recovery::fixed_view_copy::codec::{
    primitives::{
        Cursor, decode_accepted_fact, decode_id, decode_option_u16, decode_u16s, encode_option_u16,
        encode_u16s, length,
    },
    values::{decode_access, decode_bool, decode_constraint_key, decode_integer},
};

pub(super) fn encode_instruction(bytes: &mut Vec<u8>, instruction: &SelectedInstruction) {
    crate::codec_bytes::push_encoded(bytes, instruction.id.0.to_le_bytes());
    encode_kind(bytes, instruction.kind);
    encode_constraint_key(bytes, instruction.constraint);
    length(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        crate::codec_bytes::push_encoded(bytes, operand.operand.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, operand.virtual_register.0.to_le_bytes());
        bytes.push(operand.access.access_tag());
        crate::codec_bytes::push_encoded(bytes, operand.class.0.to_le_bytes());
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
        SelectedInstructionKind::PortWrite { .. } => 239,
        SelectedInstructionKind::RegisterAccess { .. } => 247,
        SelectedInstructionKind::AtomicFence => 238,
        SelectedInstructionKind::AtomicAccess { form } => form.machine_tag(),
        SelectedInstructionKind::PlacedScalarAccess { form, .. } => form.machine_tag(),
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => form.machine_tag(),
        SelectedInstructionKind::FloatResultPolicy { form } => form.machine_tag(),
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
        SelectedInstructionKind::Crash => 111,
        SelectedInstructionKind::HostedExitProcessI32 => 31,
        SelectedInstructionKind::ByteViewAddress => 22,
        SelectedInstructionKind::Load8Indexed => 21,
        SelectedInstructionKind::CopyBytes => 59,
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
        SelectedInstructionKind::BitwiseXorI64 => 52,
        SelectedInstructionKind::ExactDivideU64 { .. } => 56,
        SelectedInstructionKind::WrappingRemainderI64 { .. } => 57,
        SelectedInstructionKind::WrappingAddI64 => 58,
        // The saturating tags are the shared family tags: the u64 and i32
        // forms keep 54, 55, 60, 61, 62 and every other carrier is appended.
        SelectedInstructionKind::SaturatingAdd { carrier } => {
            saturating_family_tag(SaturatingOperation::Add, carrier)
        }
        SelectedInstructionKind::SaturatingSubtract { carrier } => {
            saturating_family_tag(SaturatingOperation::Subtract, carrier)
        }
        SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
            saturating_family_tag(SaturatingOperation::Divide, carrier)
        }
        SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
            saturating_family_tag(SaturatingOperation::Remainder, carrier)
        }
        SelectedInstructionKind::SaturatingMultiply { carrier } => {
            saturating_family_tag(SaturatingOperation::Multiply, carrier)
        }
        // Trapping forms take the shared family tags, 122 plus the form's
        // dense ordinal, and carry no payload.
        SelectedInstructionKind::TrappingInteger { form } => trapping_family_tag(form),
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
        SelectedInstructionKind::CompareI64Immediate { .. } => 53,
        SelectedInstructionKind::ConditionalBranchU64LessThan => 11,
        SelectedInstructionKind::CallScalar { .. } => 12,
        SelectedInstructionKind::ConditionalBranchI64LessThan => 13,
        SelectedInstructionKind::NormalizedForeignCall { .. } => 87,
        SelectedInstructionKind::ImmutableImageAddress { .. } => 232,
        SelectedInstructionKind::ScanByteSequence { .. } => 233,
        SelectedInstructionKind::LoadEncodedScalar => 236,
        SelectedInstructionKind::StoreEncodedScalar => 237,
        SelectedInstructionKind::CompareByteSequences => 234,
        SelectedInstructionKind::ZeroBytes => 235,
        SelectedInstructionKind::ExactMultiplyI64 { .. } => 88,
        SelectedInstructionKind::ExactRemainderU64 { .. } => 89,
        SelectedInstructionKind::WrappingSubtractI64 => 90,
        SelectedInstructionKind::WrappingMultiplyI64 => 91,
        SelectedInstructionKind::WrappingDivideI64 { .. } => 92,
        SelectedInstructionKind::ExactDivideI64 { .. } => 112,
        SelectedInstructionKind::ExactRemainderI64 { .. } => 113,
        SelectedInstructionKind::BitwiseOrI64 => 93,
        SelectedInstructionKind::BitwiseNotI64 => 94,
        SelectedInstructionKind::SaveFloatingControl { .. } => 103,
        SelectedInstructionKind::RestoreFloatingControl { .. } => 104,
        SelectedInstructionKind::WrappingShiftLeftI64 => 105,
        SelectedInstructionKind::WrappingShiftRightI64 => 106,
        SelectedInstructionKind::WrappingShiftRightU64 => 107,
        SelectedInstructionKind::ExactShiftLeftI64 { .. } => 108,
        SelectedInstructionKind::ExactShiftRightI64 { .. } => 109,
        SelectedInstructionKind::ExactShiftRightU64 { .. } => 110,
    };
    bytes.push(tag);
    match kind {
        SelectedInstructionKind::RegisterAccess { operation } => {
            bytes.extend_from_slice(&operation.catalog_tag().to_le_bytes());
        }
        SelectedInstructionKind::PortWrite { port, value } => {
            bytes.extend_from_slice(&port.to_le_bytes());
            bytes.push(value);
        }
        // Every float form shares one tag; its dense ordinal follows.
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => {
            bytes.push(form.machine_payload())
        }
        SelectedInstructionKind::PlacedScalarAccess {
            form,
            contract,
            byte_offset,
        } => {
            bytes.push(form.machine_payload());
            bytes.extend_from_slice(&contract);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::FloatResultPolicy { form } => bytes.push(form.machine_payload()),
        SelectedInstructionKind::LoadPacked { byte_offset, width }
        | SelectedInstructionKind::StorePacked { byte_offset, width } => {
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            bytes.push(width.byte_size());
        }
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            bytes.push(byte_size);
        }
        SelectedInstructionKind::AddressOffset { byte_offset } => {
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::Load64 { byte_offset }
        | selected_instructions::load_width_triple_pattern!(byte_offset, ..) => {
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
                    crate::codec_bytes::push_encoded(bytes, parameter_index.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, abi_stack_byte_offset.to_le_bytes());
                }
                selected_instructions::FrameStorageSlotId::Outgoing(slot) => {
                    bytes.push(match slot.role {
                        selected_instructions::OutgoingArgumentSlotRole::Argument => 0,
                        selected_instructions::OutgoingArgumentSlotRole::ValueCopy => 3,
                    });
                    slot.invocation_site.encode_identity(bytes);
                    crate::codec_bytes::push_encoded(bytes, slot.argument_index.to_le_bytes());
                }
                selected_instructions::FrameStorageSlotId::Local(slot) => {
                    bytes.push(1);
                    slot.encode_identity(bytes);
                }
            }
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        selected_instructions::hosted_slot_quad_pattern!(slot, ..) => slot.encode_identity(bytes),
        SelectedInstructionKind::MaterializeI64 { value } => encode_integer(bytes, value),
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            encode_integer(bytes, immediate)
        }
        selected_instructions::exact_wrapping_divrem_set_pattern!(
            obligation,
            accepted_fact,
            ..
        )
        | SelectedInstructionKind::SaturatingDivide {
            obligation,
            accepted_fact,
            ..
        }
        | SelectedInstructionKind::SaturatingRemainder {
            obligation,
            accepted_fact,
            ..
        }
        | SelectedInstructionKind::ExactDivideU64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactMultiplyI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactShiftLeftI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactShiftRightI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactShiftRightU64 {
            obligation,
            accepted_fact,
        } => {
            crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        selected_instructions::exact_add_or_sub_pair_pattern!(
            immediate,
            obligation,
            accepted_fact,
            ..
        ) => {
            encode_integer(bytes, immediate);
            crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        selected_instructions::selected_call_triple_pattern!(target, ..) => {
            target.encode_identity(bytes);
        }
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => {
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, ordinal.to_le_bytes());
        }
        SelectedInstructionKind::ImmutableImageAddress { reference } => {
            reference.encode_identity(bytes);
        }
        SelectedInstructionKind::ScanByteSequence { predicate } => {
            bytes.push(predicate.mask_bit());
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
        (SelectedInstructionKind::BitwiseXorI64, 52),
        (
            SelectedInstructionKind::SaturatingSubtract {
                carrier: selected_instructions::SaturatingCarrier::U64,
            },
            54,
        ),
        (
            SelectedInstructionKind::SaturatingAdd {
                carrier: selected_instructions::SaturatingCarrier::U64,
            },
            55,
        ),
        (SelectedInstructionKind::WrappingAddI64, 58),
        (
            SelectedInstructionKind::SaturatingAdd {
                carrier: selected_instructions::SaturatingCarrier::I32,
            },
            60,
        ),
        (
            SelectedInstructionKind::SaturatingSubtract {
                carrier: selected_instructions::SaturatingCarrier::I32,
            },
            61,
        ),
        (
            SelectedInstructionKind::SaturatingAdd {
                carrier: selected_instructions::SaturatingCarrier::I8,
            },
            63,
        ),
        (
            SelectedInstructionKind::SaturatingSubtract {
                carrier: selected_instructions::SaturatingCarrier::U16,
            },
            76,
        ),
        (
            SelectedInstructionKind::SaturatingMultiply {
                carrier: selected_instructions::SaturatingCarrier::I8,
            },
            114,
        ),
        (
            SelectedInstructionKind::SaturatingMultiply {
                carrier: selected_instructions::SaturatingCarrier::U64,
            },
            121,
        ),
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
        (SelectedInstructionKind::CopyBytes, 59),
        (SelectedInstructionKind::ZeroBytes, 235),
        (SelectedInstructionKind::LoadEncodedScalar, 236),
        (SelectedInstructionKind::StoreEncodedScalar, 237),
        (SelectedInstructionKind::ByteViewAddress, 22),
    ] {
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, kind);
        assert_eq!(bytes, [tag]);
        assert_eq!(decode_kind(&mut Cursor::new(&bytes)).unwrap(), kind);
    }
}

pub(in crate::register_homes::recovery::fixed_view_copy::codec) fn decode_kind(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionKind, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        tag @ selected_instructions::PlacedScalarForm::FAMILY_TAG => {
            SelectedInstructionKind::PlacedScalarAccess {
                form: selected_instructions::PlacedScalarForm::from_machine_payload(cursor.byte()?)
                    .ok_or(FixedViewCopyDecodeError::UnknownInstructionKind(tag))?,
                contract: cursor.array()?,
                byte_offset: cursor.u32()?,
            }
        }
        tag @ selected_instructions::FloatResultPolicyForm::FAMILY_TAG => {
            SelectedInstructionKind::FloatResultPolicy {
                form: selected_instructions::FloatResultPolicyForm::from_machine_payload(
                    cursor.byte()?,
                )
                .ok_or(FixedViewCopyDecodeError::UnknownInstructionKind(tag))?,
            }
        }
        tag @ selected_instructions::IeeeFloatArithmeticForm::FAMILY_TAG => {
            SelectedInstructionKind::NearestIeeeFloatArithmetic {
                form: selected_instructions::IeeeFloatArithmeticForm::from_machine_payload(
                    cursor.byte()?,
                )
                .ok_or(FixedViewCopyDecodeError::UnknownInstructionKind(tag))?,
            }
        }
        tag if selected_instructions::AtomicForm::from_machine_tag(tag).is_some() => {
            SelectedInstructionKind::AtomicAccess {
                form: selected_instructions::AtomicForm::from_machine_tag(tag)
                    .ok_or(FixedViewCopyDecodeError::UnknownInstructionKind(tag))?,
            }
        }
        35 => SelectedInstructionKind::CallAggregate {
            target: decode_call_target(cursor)?,
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
        111 => SelectedInstructionKind::Crash,
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
                slot_tag @ (0 | 3) => selected_instructions::FrameStorageSlotId::Outgoing(
                    selected_instructions::OutgoingArgumentSlotId {
                        role: if slot_tag == 0 {
                            selected_instructions::OutgoingArgumentSlotRole::Argument
                        } else {
                            selected_instructions::OutgoingArgumentSlotRole::ValueCopy
                        },
                        invocation_site: super::structural::decode_invocation_site(cursor)?,
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
            target: decode_call_target(cursor)?,
        },
        87 => SelectedInstructionKind::NormalizedForeignCall {
            boundary: decode_id(cursor, BoundaryMachineId::new)?,
            ordinal: cursor.u32()?,
        },
        232 => SelectedInstructionKind::ImmutableImageAddress {
            reference: match cursor.byte()? {
                0 => selected_instructions::SelectedImageAddressReference::ByteLiteral {
                    operation: decode_id(cursor, OperationId::new)?,
                    place: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
                },
                1 => selected_instructions::SelectedImageAddressReference::ImplementationTable {
                    table: cursor.u32()?,
                },
                tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
            },
        },
        tag @ 233 => SelectedInstructionKind::ScanByteSequence {
            predicate: {
                let mask = cursor.byte()?;
                selected_instructions::ByteSequencePredicate::ALL
                    .into_iter()
                    .find(|predicate| predicate.mask_bit() == mask)
                    .ok_or(FixedViewCopyDecodeError::UnknownInstructionKind(tag))?
            },
        },
        234 => SelectedInstructionKind::CompareByteSequences,
        236 => SelectedInstructionKind::LoadEncodedScalar,
        237 => SelectedInstructionKind::StoreEncodedScalar,
        239 => SelectedInstructionKind::PortWrite {
            port: cursor.u16()?,
            value: cursor.byte()?,
        },
        tag @ 247 => SelectedInstructionKind::RegisterAccess {
            operation: selected_instructions::RegisterOperation::from_catalog_tag(cursor.u16()?)
                .ok_or(FixedViewCopyDecodeError::UnknownInstructionKind(tag))?,
        },
        238 => SelectedInstructionKind::AtomicFence,
        235 => SelectedInstructionKind::ZeroBytes,
        14 => SelectedInstructionKind::Jump,
        0 => SelectedInstructionKind::CompareI64Zero,
        1 => SelectedInstructionKind::MaterializeI64 {
            value: decode_integer(cursor)?,
        },
        2 => SelectedInstructionKind::ConditionalBranchNonZero,
        3 => SelectedInstructionKind::ReturnScalar,
        4 => SelectedInstructionKind::CopyI64,
        51 => SelectedInstructionKind::BitwiseAndI64,
        52 => SelectedInstructionKind::BitwiseXorI64,
        58 => SelectedInstructionKind::WrappingAddI64,
        tag if trapping_form(tag).is_some() => SelectedInstructionKind::TrappingInteger {
            form: trapping_form(tag).unwrap(),
        },
        tag if saturating_kind(tag).is_some() => match saturating_kind(tag).unwrap() {
            (SaturatingOperation::Add, carrier) => {
                SelectedInstructionKind::SaturatingAdd { carrier }
            }
            (SaturatingOperation::Subtract, carrier) => {
                SelectedInstructionKind::SaturatingSubtract { carrier }
            }
            (SaturatingOperation::Divide, carrier) => SelectedInstructionKind::SaturatingDivide {
                carrier,
                obligation: decode_id(cursor, ObligationId::new)?,
                accepted_fact: decode_accepted_fact(cursor)?,
            },
            (SaturatingOperation::Remainder, carrier) => {
                SelectedInstructionKind::SaturatingRemainder {
                    carrier,
                    obligation: decode_id(cursor, ObligationId::new)?,
                    accepted_fact: decode_accepted_fact(cursor)?,
                }
            }
            (SaturatingOperation::Multiply, carrier) => {
                SelectedInstructionKind::SaturatingMultiply { carrier }
            }
        },
        56 => SelectedInstructionKind::ExactDivideU64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        57 => SelectedInstructionKind::WrappingRemainderI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        88 => SelectedInstructionKind::ExactMultiplyI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        89 => SelectedInstructionKind::ExactRemainderU64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        90 => SelectedInstructionKind::WrappingSubtractI64,
        91 => SelectedInstructionKind::WrappingMultiplyI64,
        92 => SelectedInstructionKind::WrappingDivideI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        112 => SelectedInstructionKind::ExactDivideI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        113 => SelectedInstructionKind::ExactRemainderI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        93 => SelectedInstructionKind::BitwiseOrI64,
        94 => SelectedInstructionKind::BitwiseNotI64,
        105 => SelectedInstructionKind::WrappingShiftLeftI64,
        106 => SelectedInstructionKind::WrappingShiftRightI64,
        107 => SelectedInstructionKind::WrappingShiftRightU64,
        tag @ (108..=110) => {
            let obligation = decode_id(cursor, ObligationId::new)?;
            let accepted_fact =
                optimization_core::AcceptedObligationFactIdentity::from_bytes(cursor.array()?);
            match tag {
                108 => SelectedInstructionKind::ExactShiftLeftI64 {
                    obligation,
                    accepted_fact,
                },
                109 => SelectedInstructionKind::ExactShiftRightI64 {
                    obligation,
                    accepted_fact,
                },
                _ => SelectedInstructionKind::ExactShiftRightU64 {
                    obligation,
                    accepted_fact,
                },
            }
        }
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
        103 => SelectedInstructionKind::SaveFloatingControl {
            slot: super::structural::decode_local_slot(cursor)?,
        },
        104 => SelectedInstructionKind::RestoreFloatingControl {
            slot: super::structural::decode_local_slot(cursor)?,
        },
        32 => SelectedInstructionKind::HostedReadByte {
            slot: super::structural::decode_local_slot(cursor)?,
        },
        23 => SelectedInstructionKind::HostedWriteByteI32 {
            slot: super::structural::decode_local_slot(cursor)?,
        },
        22 => SelectedInstructionKind::ByteViewAddress,
        21 => SelectedInstructionKind::Load8Indexed,
        59 => SelectedInstructionKind::CopyBytes,
        5 => SelectedInstructionKind::ExactAddI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        6 => SelectedInstructionKind::ExactAddI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        7 => SelectedInstructionKind::ExactSubtractI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        8 => SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        },
        9 => SelectedInstructionKind::ReturnUnit,
        10 => SelectedInstructionKind::CompareI64,
        53 => SelectedInstructionKind::CompareI64Immediate {
            immediate: decode_integer(cursor)?,
        },
        11 => SelectedInstructionKind::ConditionalBranchU64LessThan,
        12 => SelectedInstructionKind::CallScalar {
            target: decode_call_target(cursor)?,
        },
        13 => SelectedInstructionKind::ConditionalBranchI64LessThan,
        tag => return Err(FixedViewCopyDecodeError::UnknownInstructionKind(tag)),
    })
}

#[cfg(test)]
mod tests {
    use super::{Cursor, MachineId, SelectedInstructionKind};
    use crate::register_homes::recovery::fixed_view_copy::codec::selected::decode_kind;
    use crate::register_homes::recovery::fixed_view_copy::codec::selected::instruction::encode_kind;

    #[test]
    fn float_result_policy_recovery_binds_exact_payload_and_rejects_unknown_forms() {
        use crate::FixedViewCopyDecodeError;
        use selected_instructions::FloatResultPolicyForm;

        for (payload, form) in FloatResultPolicyForm::ALL.into_iter().enumerate() {
            let kind = SelectedInstructionKind::FloatResultPolicy { form };
            let mut bytes = Vec::new();
            encode_kind(&mut bytes, kind);
            assert_eq!(bytes, [231, payload as u8]);
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_kind(&mut cursor).unwrap(), kind);
            assert_eq!(cursor.remaining(), 0);
        }
        for payload in 6..=u8::MAX {
            assert_eq!(
                decode_kind(&mut Cursor::new(&[231, payload])),
                Err(FixedViewCopyDecodeError::UnknownInstructionKind(231))
            );
        }
        assert_eq!(
            decode_kind(&mut Cursor::new(&[231])),
            Err(FixedViewCopyDecodeError::Truncated)
        );
    }

    #[test]
    fn float_arithmetic_forms_keep_their_shared_tags_through_recovery() {
        for form in selected_instructions::IeeeFloatArithmeticForm::ALL {
            let kind = SelectedInstructionKind::NearestIeeeFloatArithmetic { form };
            let mut bytes = Vec::new();
            encode_kind(&mut bytes, kind);
            assert_eq!(bytes, [form.machine_tag(), form.machine_payload()]);
            assert_eq!(decode_kind(&mut Cursor::new(&bytes)).unwrap(), kind);
        }
    }

    #[test]
    fn atomic_forms_keep_their_dense_shared_tags_through_recovery() {
        for form in selected_instructions::AtomicForm::ALL {
            let kind = SelectedInstructionKind::AtomicAccess { form };
            let mut bytes = Vec::new();
            encode_kind(&mut bytes, kind);
            assert_eq!(bytes, [form.machine_tag()]);
            assert_eq!(decode_kind(&mut Cursor::new(&bytes)).unwrap(), kind);
        }
    }

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
    fn compare_i64_immediate_uses_tag_fifty_three_and_binds_immediate() {
        let first = SelectedInstructionKind::CompareI64Immediate {
            immediate: semantic_vocabulary::IntegerValue::Unsigned(0),
        };
        let second = SelectedInstructionKind::CompareI64Immediate {
            immediate: semantic_vocabulary::IntegerValue::Unsigned(4095),
        };
        let mut first_bytes = Vec::new();
        let mut second_bytes = Vec::new();
        encode_kind(&mut first_bytes, first);
        encode_kind(&mut second_bytes, second);
        assert_eq!(first_bytes[0], 53);
        assert_eq!(second_bytes[0], 53);
        assert_ne!(first_bytes, second_bytes);
        assert_eq!(decode_kind(&mut Cursor::new(&first_bytes)).unwrap(), first);
        assert_eq!(
            decode_kind(&mut Cursor::new(&second_bytes)).unwrap(),
            second
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
        let target =
            selected_instructions::SelectedCallTarget::Machine(MachineId::new(47).unwrap());
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, SelectedInstructionKind::CallScalar { target });
        assert_eq!(bytes[0], 12);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            decode_kind(&mut cursor).unwrap(),
            SelectedInstructionKind::CallScalar { target }
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
        role: selected_instructions::OutgoingArgumentSlotRole::Argument,
        invocation_site: semantic_vocabulary::OperationId::new(43).unwrap().into(),
        argument_index: 1,
    };
    for kind in [
        SelectedInstructionKind::HostedReadByte {
            slot: selected_instructions::LocalStorageSlotId::Structural {
                operation: slot
                    .invocation_site
                    .operation()
                    .expect("operation-backed test slot"),
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
                operation: slot
                    .invocation_site
                    .operation()
                    .expect("operation-backed test slot"),
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
            target: selected_instructions::SelectedCallTarget::Machine(MachineId::new(47).unwrap()),
        },
    ] {
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, kind);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_kind(&mut cursor).unwrap(), kind);
        assert_eq!(cursor.remaining(), 0);
    }
}

#[test]
#[cfg(test)]
fn outgoing_slot_roles_round_trip_with_legacy_argument_frame_bytes() {
    use selected_instructions::{
        FrameStorageSlotId, OutgoingArgumentSlotId, OutgoingArgumentSlotRole,
    };
    let argument = OutgoingArgumentSlotId {
        invocation_site: (OperationId::new(43).unwrap()).into(),
        argument_index: 7,
        role: OutgoingArgumentSlotRole::Argument,
    };
    for (role, frame_tag) in [
        (OutgoingArgumentSlotRole::Argument, 0),
        (OutgoingArgumentSlotRole::ValueCopy, 3),
    ] {
        let slot = FrameStorageSlotId::Outgoing(OutgoingArgumentSlotId { role, ..argument });
        for (kind, tag) in [
            (
                SelectedInstructionKind::Store64 {
                    slot,
                    byte_offset: 8,
                },
                17,
            ),
            (
                SelectedInstructionKind::FrameAddress {
                    slot,
                    byte_offset: 8,
                },
                18,
            ),
        ] {
            let mut expected = vec![tag, frame_tag];
            crate::codec_bytes::push_encoded(&mut expected, 43_u64.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut expected, 7_u32.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut expected, 8_u32.to_le_bytes());
            let mut bytes = Vec::new();
            encode_kind(&mut bytes, kind);
            assert_eq!(bytes, expected);
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_kind(&mut cursor).unwrap(), kind);
            assert_eq!(cursor.remaining(), 0);
            bytes[1] = 4;
            assert_eq!(
                decode_kind(&mut Cursor::new(&bytes)),
                Err(FixedViewCopyDecodeError::UnknownOption(4))
            );
        }
    }
}

#[test]
#[cfg(test)]
fn wrapping_remainder_round_trips_with_distinct_tag_and_complete_proof() {
    use optimization_core::AcceptedObligationFactIdentity;

    let obligation = ObligationId::new(41).unwrap();
    let accepted_fact = AcceptedObligationFactIdentity::from_bytes([42; 32]);
    for (kind, tag) in [
        (
            SelectedInstructionKind::ExactDivideU64 {
                obligation,
                accepted_fact,
            },
            56,
        ),
        (
            SelectedInstructionKind::WrappingRemainderI64 {
                obligation,
                accepted_fact,
            },
            57,
        ),
        (
            SelectedInstructionKind::SaturatingDivide {
                carrier: selected_instructions::SaturatingCarrier::I32,
                obligation,
                accepted_fact,
            },
            62,
        ),
        (
            SelectedInstructionKind::SaturatingDivide {
                carrier: selected_instructions::SaturatingCarrier::U64,
                obligation,
                accepted_fact,
            },
            86,
        ),
    ] {
        let mut expected = vec![tag];
        crate::codec_bytes::push_encoded(&mut expected, obligation.get().to_le_bytes());
        expected.extend_from_slice(&accepted_fact.bytes());
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, kind);
        assert_eq!(bytes, expected);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_kind(&mut cursor).unwrap(), kind);
        assert_eq!(cursor.remaining(), 0);
    }
}

/// The Trapping form a tag names, inverting the shared family tag table.
fn trapping_form(tag: u8) -> Option<TrappingForm> {
    TrappingForm::ALL
        .into_iter()
        .find(|form| trapping_family_tag(*form) == tag)
}

fn decode_call_target(
    cursor: &mut Cursor<'_>,
) -> Result<selected_instructions::SelectedCallTarget, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        0 => selected_instructions::SelectedCallTarget::Machine(decode_id(cursor, MachineId::new)?),
        1 => selected_instructions::SelectedCallTarget::Dynamic {
            interface: cursor.u32()?,
            slot: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    })
}
