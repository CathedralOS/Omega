//! Optimizer module role: named leaf. The one owner of a selected instruction
//! kind's row: its tag and its operand payload.
//!
//! Two envelopes persist selected instructions — the pre-allocation effect
//! identity and the fixed-view-copy recovery codec — and each versions its own
//! kind tag table. The tag is therefore the caller's decision; the payload that
//! follows it is one shared row. Both are written here, in that order, so every
//! envelope spells a kind's payload once and no codec can drift from another.

use crate::SelectedInstructionKind;

/// Appends `tag` and the exact operand payload `kind` carries. Each caller owns
/// its tag table; none authors the payload separately.
pub fn encode_kind(bytes: &mut Vec<u8>, tag: u8, kind: SelectedInstructionKind) {
    bytes.push(tag);
    match kind {
        SelectedInstructionKind::RegisterAccess { operation } => {
            crate::codec_bytes::push_encoded(bytes, operation.catalog_tag().to_le_bytes());
        }
        SelectedInstructionKind::PortWrite { port, value } => {
            crate::codec_bytes::push_encoded(bytes, port.to_le_bytes());
            bytes.push(value);
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
        SelectedInstructionKind::MaterializeI64 { value } => {
            legalized_operations::encode_integer(bytes, value)
        }
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
            legalized_operations::encode_integer(bytes, immediate);
            crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            legalized_operations::encode_integer(bytes, immediate)
        }
        // Every float form shares one tag; its dense ordinal follows.
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
        SelectedInstructionKind::FloatResultPolicy { form } => bytes.push(form.machine_payload()),
        SelectedInstructionKind::CompareI64Zero
        | SelectedInstructionKind::CopyBytes
        | SelectedInstructionKind::ZeroBytes
        | SelectedInstructionKind::TrappingInteger { .. }
        | SelectedInstructionKind::AtomicFence
        | SelectedInstructionKind::AtomicAccess { .. }
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
