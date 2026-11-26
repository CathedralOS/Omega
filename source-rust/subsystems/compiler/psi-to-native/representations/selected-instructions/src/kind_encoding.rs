/*
  Shared canonical byte payload encoder for `SelectedInstructionKind`.

  Both the pre-allocation effect identity and the fixed-view-copy recovery
  codec persist selected instructions with their own versioned kind tag tables
  followed by the identical operand payload. `encode_kind` writes the caller's
  tag and the exact payload of `kind` in one place so no envelope can drift
  from another.
*/

use crate::SelectedInstructionKind;
use target_vocabulary::encode_integer;

pub fn encode_kind(bytes: &mut Vec<u8>, tag: u8, kind: SelectedInstructionKind) {
    bytes.push(tag);
    match kind {
        SelectedInstructionKind::InstalledInterruptCpuCheck { gdt_bytes }
        | SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes } => {
            crate::codec_bytes::push_encoded(bytes, gdt_bytes.to_le_bytes());
        }
        SelectedInstructionKind::InstalledInterruptTablePublication { table_bytes } => {
            crate::codec_bytes::push_encoded(bytes, table_bytes.to_le_bytes());
        }
        SelectedInstructionKind::RegisterAccess { operation } => {
            crate::codec_bytes::push_encoded(bytes, operation.catalog_tag().to_le_bytes());
        }
        SelectedInstructionKind::PortWrite {
            service,
            port,
            value,
        } => {
            crate::codec_bytes::push_encoded(bytes, port.to_le_bytes());
            bytes.push(value);
            crate::codec_bytes::push_encoded(bytes, service.get().to_le_bytes());
        }
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
        | crate::load_width_triple_pattern!(byte_offset, ..) => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes())
        }
        SelectedInstructionKind::Store64 { slot, byte_offset }
        | SelectedInstructionKind::FrameAddress { slot, byte_offset } => {
            match slot {
                crate::FrameStorageSlotId::ActivationBase => bytes.push(4),
                crate::FrameStorageSlotId::Incoming {
                    parameter_index,
                    abi_stack_byte_offset,
                } => {
                    bytes.push(2);
                    crate::codec_bytes::push_encoded(bytes, parameter_index.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, abi_stack_byte_offset.to_le_bytes());
                }
                crate::FrameStorageSlotId::Outgoing(slot) => {
                    bytes.push(match slot.role {
                        crate::OutgoingArgumentSlotRole::Argument => 0,
                        crate::OutgoingArgumentSlotRole::ValueCopy => 3,
                    });
                    slot.invocation_site.encode_identity(bytes);
                    crate::codec_bytes::push_encoded(bytes, slot.argument_index.to_le_bytes());
                }
                crate::FrameStorageSlotId::Local(slot) => {
                    bytes.push(1);
                    slot.encode_identity(bytes);
                }
            }
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        crate::hosted_slot_quad_pattern!(slot, ..) => slot.encode_identity(bytes),
        SelectedInstructionKind::MaterializeI64 { value } => encode_integer(bytes, value),
        SelectedInstructionKind::WrappingRemainderI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::WrappingDivideI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactDivideI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactRemainderI64 {
            obligation,
            accepted_fact,
        }
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
        | SelectedInstructionKind::ExactRemainderU64 {
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
        crate::exact_add_or_sub_pair_pattern!(immediate, obligation, accepted_fact, ..) => {
            encode_integer(bytes, immediate);
            crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            encode_integer(bytes, immediate)
        }
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => {
            bytes.push(form.machine_payload())
        }
        SelectedInstructionKind::AccessorScalarAccess {
            form,
            contract,
            byte_offset,
        } => {
            bytes.push(form.machine_payload());
            bytes.extend_from_slice(&contract);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::Load16Indexed => bytes.push(2),
        SelectedInstructionKind::Load32Indexed => bytes.push(4),
        SelectedInstructionKind::Load64Indexed => bytes.push(8),
        SelectedInstructionKind::FloatResultPolicy { form } => bytes.push(form.machine_payload()),
        SelectedInstructionKind::AtomicAccess { form } => {
            if let Some(payload) = form.machine_payload() {
                bytes.push(payload);
            }
        }
        SelectedInstructionKind::PackedSubI16x8 => bytes.push(0),
        SelectedInstructionKind::PackedShuffleI16x8 { lanes } => {
            bytes.push(1);
            bytes.extend_from_slice(&lanes);
        }
        SelectedInstructionKind::PackedShuffleGeneralI16x8 { lanes } => {
            bytes.push(2);
            bytes.extend_from_slice(&lanes);
        }
        SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { count } => {
            bytes.push(3);
            bytes.push(count);
        }
        SelectedInstructionKind::PackedWidenU8x8ToI16x8 => bytes.push(4),
        SelectedInstructionKind::PackedNarrowI16x8ToU8x8Saturating => bytes.push(5),
        SelectedInstructionKind::PackedLoadI16x4ZeroExtend => bytes.push(6),
        SelectedInstructionKind::PackedWidenU8x4ToI16x8 => bytes.push(7),
        SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating => bytes.push(8),
        SelectedInstructionKind::PackedLoad128 { byte_offset } => {
            bytes.push(9);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::PackedStore128 { byte_offset } => {
            bytes.push(10);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::PackedLoad64ZeroExtend { byte_offset } => {
            bytes.push(11);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::PackedStore64 { byte_offset } => {
            bytes.push(12);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::PackedLoad32ZeroExtend { byte_offset } => {
            bytes.push(13);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::PackedStore32 { byte_offset } => {
            bytes.push(14);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::PackedMaterializeZero => bytes.push(15),
        SelectedInstructionKind::PackedFromGpr64ZeroExtend => bytes.push(16),
        SelectedInstructionKind::PackedFromGpr64Pair => bytes.push(17),
        SelectedInstructionKind::PackedAddI16x8Reg => bytes.push(18),
        SelectedInstructionKind::PackedSubI16x8Reg => bytes.push(19),
        SelectedInstructionKind::PackedShiftRightArithmeticI16x8Reg { count } => {
            bytes.push(20);
            bytes.push(count);
        }
        SelectedInstructionKind::PackedShuffleI16x8Reg { lanes } => {
            bytes.push(21);
            bytes.extend_from_slice(&lanes);
        }
        SelectedInstructionKind::PackedWidenU8x8ToI16x8Reg => bytes.push(22),
        SelectedInstructionKind::PackedNarrowI16x8ToU8x8SaturatingReg => bytes.push(23),
        SelectedInstructionKind::PackedAddU8x16 => bytes.push(24),
        SelectedInstructionKind::PackedMinI16x8 => bytes.push(25),
        SelectedInstructionKind::PackedCmpGtI16x8 => bytes.push(26),
        SelectedInstructionKind::PackedBitAndI16x8 => bytes.push(27),
        SelectedInstructionKind::PackedBitAndNotI16x8 => bytes.push(28),
        SelectedInstructionKind::PackedAbsI16x8 => bytes.push(29),
        SelectedInstructionKind::PackedAddU8x16Reg => bytes.push(30),
        SelectedInstructionKind::PackedMinI16x8Reg => bytes.push(31),
        SelectedInstructionKind::PackedCmpGtI16x8Reg => bytes.push(32),
        SelectedInstructionKind::PackedBitAndI16x8Reg => bytes.push(33),
        SelectedInstructionKind::PackedBitAndNotI16x8Reg => bytes.push(34),
        SelectedInstructionKind::PackedAbsI16x8Reg => bytes.push(35),
        SelectedInstructionKind::CompareI64Zero
        | SelectedInstructionKind::PackedAddI16x8
        | SelectedInstructionKind::CopyBytes
        | SelectedInstructionKind::ZeroBytes
        | SelectedInstructionKind::TrappingInteger { .. }
        | SelectedInstructionKind::AtomicFence
        | SelectedInstructionKind::CompareI64
        | SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::BitwiseAndI64
        | SelectedInstructionKind::BitwiseOrI64
        | SelectedInstructionKind::BitwiseXorI64
        | SelectedInstructionKind::BitwiseNotI64
        | SelectedInstructionKind::WrappingAddI64
        | SelectedInstructionKind::WrappingSubtractI64
        | SelectedInstructionKind::WrappingMultiplyI64
        | SelectedInstructionKind::WrappingShiftLeftI64
        | SelectedInstructionKind::WrappingShiftRightI64
        | SelectedInstructionKind::WrappingShiftRightU64
        | crate::saturating_add_or_subtract_pattern!()
        | SelectedInstructionKind::SaturatingMultiply { .. }
        | SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::ZeroExtendU8
        | SelectedInstructionKind::ZeroExtendU32
        | SelectedInstructionKind::ZeroExtendU16
        | SelectedInstructionKind::SignExtendI8
        | SelectedInstructionKind::SignExtendI16
        | SelectedInstructionKind::SignExtendI32
        | SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
        | SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump
        | SelectedInstructionKind::Crash
        | SelectedInstructionKind::PrivilegedCrash
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::ReturnUnit
        | SelectedInstructionKind::CompareByteSequences
        | SelectedInstructionKind::LoadEncodedScalar
        | SelectedInstructionKind::StoreEncodedScalar => {}
        SelectedInstructionKind::ScanByteSequence { predicate } => {
            bytes.push(predicate.mask_bit());
        }
        SelectedInstructionKind::ReturnAggregate { fragment_count } => bytes.push(fragment_count),
        crate::selected_call_pattern!(target, ..) => {
            target.encode_identity(bytes);
        }
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => {
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, ordinal.to_le_bytes());
        }
        SelectedInstructionKind::ImageAddress { reference } => {
            reference.encode_identity(bytes);
        }
    }
}
