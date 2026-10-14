use register_model::encode_constraint_key;

use crate::{
    MachineAlternativeApplicability, MachineAlternativeFamily, MachineBarrier,
    MachineEffectCatalog, MachineEffectCatalogIdentity, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineSemanticKind, MachineSizeKnowledge,
    SaturatingCarrier, SaturatingOperation,
};

/// The sink a canonical machine identity encoding writes its bytes into.
///
/// Every encoder below produces one exact byte sequence. Whether the consumer
/// accumulates that sequence in a buffer or feeds it straight into a hasher
/// must not be able to change it, so consumers implement this sink instead of
/// keeping a second, hasher-shaped copy of the encoder. A second copy is how
/// two stages that are required to agree drift apart silently: both keep
/// compiling, both keep producing bytes, and only a rejected artifact identity
/// ever reveals it.
pub trait MachineIdentityBytes {
    fn identity_bytes(&mut self, bytes: &[u8]);
}

impl MachineIdentityBytes for Vec<u8> {
    fn identity_bytes(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

impl MachineIdentityBytes for sha2::Sha256 {
    fn identity_bytes(&mut self, bytes: &[u8]) {
        sha2::Digest::update(self, bytes);
    }
}

impl MachineIdentityBytes for optimization_core::digest_encoder::DomainDigestEncoder {
    fn identity_bytes(&mut self, bytes: &[u8]) {
        self.raw(bytes);
    }
}

pub fn machine_effect_catalog_identity(
    catalog: &MachineEffectCatalog,
) -> MachineEffectCatalogIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-machine-effect-catalog.v32\0");
    bytes.extend_from_slice(&catalog.target.identity_bytes());
    bytes.extend_from_slice(&catalog.register_constraints.bytes());
    for key in [
        catalog.selected_keys.atomic_fence,
        catalog.selected_keys.atomic_load,
        catalog.selected_keys.placed_read,
        catalog.selected_keys.placed_write,
        catalog.selected_keys.atomic_store,
        catalog.selected_keys.atomic_update,
        catalog.selected_keys.atomic_compare_exchange,
        catalog.selected_keys.atomic_bitwise_update,
        catalog.selected_keys.copy_bytes,
        catalog.selected_keys.hosted_read_byte,
        catalog.selected_keys.hosted_write_byte_i32,
        catalog.selected_keys.store,
        catalog.selected_keys.address_offset,
        catalog.selected_keys.load64,
        catalog.selected_keys.load_packed,
        catalog.selected_keys.store_packed,
        catalog.selected_keys.load8,
        catalog.selected_keys.load16,
        catalog.selected_keys.load32,
        catalog.selected_keys.load8_indexed,
        catalog.selected_keys.store64,
        catalog.selected_keys.frame_address,
        catalog.selected_keys.byte_literal_image_address,
        catalog.selected_keys.float32_to_bits,
        catalog.selected_keys.float64_to_bits,
        catalog.selected_keys.bits_to_float32,
        catalog.selected_keys.bits_to_float64,
        catalog.selected_keys.save_floating_control,
        catalog.selected_keys.restore_floating_control,
        catalog.selected_keys.float_result_trapping,
        catalog.selected_keys.float_result_saturating,
    ] {
        bytes.push(u8::from(key.is_some()));
    }
    encode_len(&mut bytes, catalog.selected_keys.call_unit.len());
    encode_len(&mut bytes, catalog.selected_keys.call_unit_mixed.len());
    encode_len(&mut bytes, catalog.selected_keys.call_scalar.len());
    encode_len(&mut bytes, catalog.selected_keys.call_aggregate.len());
    encode_len(
        &mut bytes,
        catalog.selected_keys.call_normalized_foreign.len(),
    );
    encode_len(&mut bytes, catalog.selected_keys.return_aggregate.len());
    let selected_keys = catalog.selected_keys.in_identity_order();
    encode_len(&mut bytes, selected_keys.len());
    for key in selected_keys {
        encode_constraint_key(&mut bytes, key);
    }
    encode_len(&mut bytes, catalog.declarations.len());
    for declaration in &catalog.declarations {
        bytes.push(semantic_kind_tag(declaration.semantic));
        bytes.extend(semantic_kind_payload(declaration.semantic));
        encode_constraint_key(&mut bytes, declaration.constraint);
        // Keep effect matches exhaustive so additions cannot silently collide
        // with an older catalog identity.
        bytes.push(match declaration.memory {
            crate::MachineMemoryEffect::CopyBytesV1 => 6,
            crate::MachineMemoryEffect::AtomicFenceV1 => 15,
            crate::MachineMemoryEffect::AtomicPointerV1 => 8,
            crate::MachineMemoryEffect::TemporaryStackStorageV1 => 9,
            crate::MachineMemoryEffect::NoneV1 => 0,
            crate::MachineMemoryEffect::ReadPointerV1 => 1,
            crate::MachineMemoryEffect::HostedReadByteV1 => 5,
            crate::MachineMemoryEffect::HostedWriteByteV1 => 3,
            crate::MachineMemoryEffect::WriteFrameStorageV1 => 2,
            crate::MachineMemoryEffect::ReadFrameStorageV1 => 7,
            crate::MachineMemoryEffect::WritePointerV1 => 4,
            crate::MachineMemoryEffect::ScanByteSequenceV1 => 10,
            crate::MachineMemoryEffect::ReadEncodedScalarV1 => 13,
            crate::MachineMemoryEffect::WriteEncodedScalarV1 => 14,
            crate::MachineMemoryEffect::CompareByteSequencesV1 => 11,
            crate::MachineMemoryEffect::ZeroBytesV1 => 12,
        });
        bytes.push(match declaration.trap {
            crate::MachineTrapBehavior::ExplicitCrashV1 => 5,
            crate::MachineTrapBehavior::TrappingIntegerV1 => 6,
            crate::MachineTrapBehavior::NeverV1 => 0,
            crate::MachineTrapBehavior::HostedExitReturnedV1 => 3,
            crate::MachineTrapBehavior::HostedReadFailureV1 => 4,
            crate::MachineTrapBehavior::HostedWriteFailureV1 => 2,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
        });
        bytes.push(match declaration.barrier {
            MachineBarrier::None => 0,
            MachineBarrier::ControlFlow => 1,
            MachineBarrier::ExternalEffect => 3,
            MachineBarrier::Call => 2,
        });
        match declaration.call {
            crate::MachineCallEffect::NoneV1 => bytes.push(0),
            crate::MachineCallEffect::SupervisorNormalReturnV1 => bytes.push(3),
            crate::MachineCallEffect::InternalNormalReturnV1 {
                pre_call_stack_alignment,
            } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    pre_call_stack_alignment.to_le_bytes(),
                );
            }
            crate::MachineCallEffect::DirectExternalNormalReturnV1 {
                pre_call_stack_alignment,
            } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    pre_call_stack_alignment.to_le_bytes(),
                );
            }
        }
        bytes.push(match declaration.cleanup {
            crate::MachineCleanupEffect::NoneV1 => 0,
        });
        encode_len(&mut bytes, declaration.alternatives.len());
        for alternative in &declaration.alternatives {
            encode_machine_alternative_identity(&mut bytes, alternative);
        }
    }
    MachineEffectCatalogIdentity::from_canonical_bytes(&bytes)
}

/// The canonical identity encoding of a [`crate::MachineAlternative`]: its
/// key, applicability, size knowledge, latency knowledge and encoded effects,
/// in that order.
///
/// Every tag and field order here *is* the identity of the machine-effect
/// catalog, the selected-effect program identity and the in-memory
/// physical-instruction identity. Those three derived exactly these bytes from
/// three separate copies of this body; a fourth copy would let them drift
/// apart the next time an applicability, size or latency variant is added,
/// without anything failing to compile.
pub fn encode_machine_alternative_identity<Sink: MachineIdentityBytes + ?Sized>(
    bytes: &mut Sink,
    alternative: &crate::MachineAlternative,
) {
    encode_machine_alternative_key_identity(bytes, alternative.key);
    match alternative.applicability {
        MachineAlternativeApplicability::Always => bytes.identity_bytes(&[0]),
        MachineAlternativeApplicability::ResultAliasesOperand { result, operand } => {
            bytes.identity_bytes(&[1]);
            bytes.identity_bytes(&result.to_le_bytes());
            bytes.identity_bytes(&operand.to_le_bytes());
        }
        MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            bytes.identity_bytes(&[2]);
            bytes.identity_bytes(&result.to_le_bytes());
            bytes.identity_bytes(&aliased_operand.to_le_bytes());
            bytes.identity_bytes(&distinct_operand.to_le_bytes());
        }
        MachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            bytes.identity_bytes(&[3]);
            bytes.identity_bytes(&result.to_le_bytes());
            bytes.identity_bytes(&left.to_le_bytes());
            bytes.identity_bytes(&right.to_le_bytes());
        }
        MachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            bytes.identity_bytes(&[4]);
            bytes.identity_bytes(&result.to_le_bytes());
            bytes.identity_bytes(&left.to_le_bytes());
            bytes.identity_bytes(&right.to_le_bytes());
        }
        MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left,
            right,
            excluded_view,
        } => {
            bytes.identity_bytes(&[5]);
            bytes.identity_bytes(&left.to_le_bytes());
            bytes.identity_bytes(&right.to_le_bytes());
            bytes.identity_bytes(&excluded_view.0.to_le_bytes());
        }
        MachineAlternativeApplicability::OptimizationSelected { optimization } => {
            bytes.identity_bytes(&[6]);
            bytes.identity_bytes(&[optimization as u8]);
        }
    }
    match alternative.size {
        MachineSizeKnowledge::ExactBytes(count) => {
            bytes.identity_bytes(&[0]);
            bytes.identity_bytes(&count.to_le_bytes());
        }
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => {
            bytes.identity_bytes(&[1]);
            bytes.identity_bytes(&minimum_bytes.to_le_bytes());
            match maximum_bytes {
                None => bytes.identity_bytes(&[0]),
                Some(maximum) => {
                    bytes.identity_bytes(&[1]);
                    bytes.identity_bytes(&maximum.to_le_bytes());
                }
            }
        }
    }
    bytes.identity_bytes(&[match alternative.latency {
        MachineLatencyKnowledge::StableBaselineUnavailable => 0,
    }]);
    encode_machine_encoded_effects_identity(bytes, &alternative.encoded);
}

/// The canonical identity encoding of [`MachineEncodedEffects`].
///
/// Every list length, every variant tag and the little-endian field order that
/// follows each tag *is* the identity: the machine-effect catalog identity, the
/// selected-effect program identity, the in-memory physical-instruction
/// identity and the machine-code fragment, encoding, text-section and
/// relaxation identities all hash exactly this byte stream. Changing one tag
/// changes every artifact and replay record that embeds an encoded effect.
///
/// Consumers must call this function rather than repeat the table, over
/// whichever [`MachineIdentityBytes`] sink they accumulate into.
pub fn encode_machine_encoded_effects_identity<Sink: MachineIdentityBytes + ?Sized>(
    bytes: &mut Sink,
    effects: &MachineEncodedEffects,
) {
    encode_u16s(bytes, &effects.external_operand_reads);
    encode_u16s(bytes, &effects.external_operand_writes);
    encode_units(bytes, &effects.implicit_unit_uses);
    encode_units(bytes, &effects.implicit_unit_defs);
    encode_units(bytes, &effects.implicit_unit_clobbers);
    match effects.memory {
        MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.identity_bytes(&[12]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
            bytes.identity_bytes(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::HostedReadByteV1 { stack_pointer } => {
            bytes.identity_bytes(&[8]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
        }
        MachineEncodedMemoryEffect::HostedWriteByteV1 { stack_pointer } => {
            bytes.identity_bytes(&[6]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
        }
        MachineEncodedMemoryEffect::CopyBytesV1 {
            source_pointer_operand,
            destination_pointer_operand,
            count_operand,
        } => {
            bytes.identity_bytes(&[9]);
            bytes.identity_bytes(&source_pointer_operand.to_le_bytes());
            bytes.identity_bytes(&destination_pointer_operand.to_le_bytes());
            bytes.identity_bytes(&count_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
            pointer_operand,
            codec_operand,
        } => {
            bytes.identity_bytes(&[16]);
            bytes.identity_bytes(&pointer_operand.to_le_bytes());
            bytes.identity_bytes(&codec_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
            pointer_operand,
            codec_operand,
        } => {
            bytes.identity_bytes(&[17]);
            bytes.identity_bytes(&pointer_operand.to_le_bytes());
            bytes.identity_bytes(&codec_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ScanByteSequenceV1 {
            base_pointer_operand,
            count_operand,
        } => {
            bytes.identity_bytes(&[13]);
            bytes.identity_bytes(&base_pointer_operand.to_le_bytes());
            bytes.identity_bytes(&count_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::CompareByteSequencesV1 {
            left_base_operand,
            left_count_operand,
            right_base_operand,
            right_count_operand,
        } => {
            bytes.identity_bytes(&[14]);
            bytes.identity_bytes(&left_base_operand.to_le_bytes());
            bytes.identity_bytes(&left_count_operand.to_le_bytes());
            bytes.identity_bytes(&right_base_operand.to_le_bytes());
            bytes.identity_bytes(&right_count_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ZeroBytesV1 {
            destination_pointer_operand,
            count_operand,
        } => {
            bytes.identity_bytes(&[15]);
            bytes.identity_bytes(&destination_pointer_operand.to_le_bytes());
            bytes.identity_bytes(&count_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::NoneV1 => bytes.identity_bytes(&[0]),
        MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand } => {
            bytes.identity_bytes(&[7]);
            bytes.identity_bytes(&pointer_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::AtomicFenceV1 => bytes.identity_bytes(&[18]),
        MachineEncodedMemoryEffect::AtomicPointerV1 {
            pointer_operand,
            form,
        } => {
            bytes.identity_bytes(&[11]);
            bytes.identity_bytes(&pointer_operand.to_le_bytes());
            bytes.identity_bytes(&[form.ordinal()]);
        }
        MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand,
            byte_count,
        } => {
            bytes.identity_bytes(&[3]);
            bytes.identity_bytes(&pointer_operand.to_le_bytes());
            bytes.identity_bytes(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
            pointer_operand,
            index_operand,
            byte_count,
        } => {
            bytes.identity_bytes(&[5]);
            bytes.identity_bytes(&pointer_operand.to_le_bytes());
            bytes.identity_bytes(&index_operand.to_le_bytes());
            bytes.identity_bytes(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WriteFrameStorageV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.identity_bytes(&[4]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
            bytes.identity_bytes(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ReadFrameStorageV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.identity_bytes(&[10]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
            bytes.identity_bytes(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.identity_bytes(&[1]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
            bytes.identity_bytes(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.identity_bytes(&[2]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
            bytes.identity_bytes(&byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        MachineEncodedStackEffect::TemporaryBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.identity_bytes(&[3]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
            bytes.identity_bytes(&byte_count.to_le_bytes());
        }
        MachineEncodedStackEffect::UnchangedV1 => bytes.identity_bytes(&[0]),
        MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.identity_bytes(&[1]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
            bytes.identity_bytes(&byte_count.to_le_bytes());
        }
        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer,
            return_address_byte_count,
        } => {
            bytes.identity_bytes(&[2]);
            bytes.identity_bytes(&stack_pointer.0.to_le_bytes());
            bytes.identity_bytes(&return_address_byte_count.to_le_bytes());
        }
    }
    bytes.identity_bytes(&[match effects.trap {
        MachineEncodedTrapBehavior::ExplicitCrashV1 => 5,
        MachineEncodedTrapBehavior::TrappingIntegerV1 => 6,
        MachineEncodedTrapBehavior::NeverV1 => 0,
        MachineEncodedTrapBehavior::HostedExitReturnedV1 => 3,
        MachineEncodedTrapBehavior::HostedReadFailureV1 => 4,
        MachineEncodedTrapBehavior::HostedWriteFailureV1 => 2,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        MachineEncodedControlEffect::CrashV1 => bytes.identity_bytes(&[9]),
        MachineEncodedControlEffect::FallThroughOrTrapV1 => bytes.identity_bytes(&[10]),
        MachineEncodedControlEffect::HostedExitOrTrapV1 => bytes.identity_bytes(&[7]),
        MachineEncodedControlEffect::HostedReadReturnOrTrapV1 => bytes.identity_bytes(&[8]),
        MachineEncodedControlEffect::HostedWriteReturnOrTrapV1 => bytes.identity_bytes(&[6]),
        MachineEncodedControlEffect::FallThroughV1 => bytes.identity_bytes(&[0]),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1 => bytes.identity_bytes(&[1]),
        MachineEncodedControlEffect::ReturnFromActivationStackV1 => bytes.identity_bytes(&[2]),
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            bytes.identity_bytes(&[3]);
            bytes.identity_bytes(&target.0.to_le_bytes());
        }
        MachineEncodedControlEffect::DirectRelativeCallV1 => bytes.identity_bytes(&[4]),
        MachineEncodedControlEffect::IndirectTableCallOrTrapV1 => bytes.identity_bytes(&[13]),
        MachineEncodedControlEffect::SupervisorCallV1 => bytes.identity_bytes(&[11]),
        MachineEncodedControlEffect::AtomicRetryUntilCommitV1 => bytes.identity_bytes(&[12]),
        MachineEncodedControlEffect::UnconditionalRelativeBranchV1 => bytes.identity_bytes(&[5]),
    }
}

fn encode_u16s<Sink: MachineIdentityBytes + ?Sized>(bytes: &mut Sink, values: &[u16]) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.identity_bytes(&value.to_le_bytes());
    }
}

fn encode_units<Sink: MachineIdentityBytes + ?Sized>(
    bytes: &mut Sink,
    units: &[register_model::RegisterUnitId],
) {
    encode_len(bytes, units.len());
    for unit in units {
        bytes.identity_bytes(&unit.0.to_le_bytes());
    }
}

macro_rules! effect_tag_arms {
    ($enum:ident, $value:expr) => {
        match $value {
            $enum::CopyBytes => 59,
            $enum::ScanByteSequence => 233,
            $enum::LoadEncodedScalar => 236,
            $enum::StoreEncodedScalar => 237,
            $enum::CompareByteSequences => 234,
            $enum::ZeroBytes => 235,
            $enum::CallAggregate => 35,
            $enum::ReturnAggregate => 36,
            $enum::CompareI64Zero => 0,
            $enum::MaterializeI64 => 1,
            $enum::CopyI64 => 2,
            $enum::Float32ToBits => 26,
            $enum::Float64ToBits => 27,
            $enum::BitsToFloat32 => 28,
            $enum::BitsToFloat64 => 29,
            $enum::ZeroExtendU8 => 15,
            $enum::ZeroExtendU32 => 20,
            $enum::ZeroExtendU16 => 37,
            $enum::SignExtendI8 => 38,
            $enum::SignExtendI16 => 39,
            $enum::SignExtendI32 => 40,
            $enum::MaterializeBooleanEqual => 41,
            $enum::MaterializeBooleanU64LessThan => 42,
            $enum::MaterializeBooleanI64LessThan => 43,
            $enum::MaterializeBooleanU64LessOrEqual => 44,
            $enum::MaterializeBooleanI64LessOrEqual => 45,
            $enum::Load64 => 16,
            $enum::LoadPacked3 => 46,
            $enum::LoadPacked5 => 47,
            $enum::LoadPacked6 => 48,
            $enum::LoadPacked7 => 49,
            $enum::StorePacked => 50,
            $enum::Load8 => 33,
            $enum::Load16 => 34,
            $enum::Load32 => 30,
            $enum::Crash => 111,
            $enum::HostedExitProcessI32 => 31,
            $enum::HostedReadByte => 32,
            $enum::HostedWriteByteI32 => 23,
            $enum::Store => 24,
            $enum::AddressOffset => 25,
            $enum::ByteViewAddress => 22,
            $enum::Load8Indexed => 21,
            $enum::Store64 => 17,
            $enum::FrameAddress => 18,
            $enum::ImmutableImageAddress => 232,
            $enum::CallUnit => 19,
            $enum::ExactAddI64 => 3,
            $enum::BitwiseAndI64 => 51,
            $enum::BitwiseXorI64 => 52,
            $enum::ExactDivideU64 => 56,
            $enum::WrappingRemainderI64 => 57,
            $enum::WrappingAddI64 => 58,
            $enum::SaturatingAdd(carrier) => {
                saturating_family_tag(SaturatingOperation::Add, carrier)
            }
            $enum::SaturatingSubtract(carrier) => {
                saturating_family_tag(SaturatingOperation::Subtract, carrier)
            }
            $enum::SaturatingDivide(carrier) => {
                saturating_family_tag(SaturatingOperation::Divide, carrier)
            }
            $enum::SaturatingRemainder(carrier) => {
                saturating_family_tag(SaturatingOperation::Remainder, carrier)
            }
            $enum::ExactAddI64Immediate => 4,
            $enum::ExactSubtractI64 => 5,
            $enum::ConditionalBranchNonZero => 6,
            $enum::ReturnScalar => 7,
            $enum::ExactSubtractI64Immediate => 8,
            $enum::ReturnUnit => 9,
            $enum::CompareI64 => 10,
            $enum::CompareI64Immediate => 53,
            $enum::ConditionalBranchU64LessThan => 11,
            $enum::ConditionalBranchI64LessThan => 12,
            $enum::CallScalar => 13,
            $enum::Jump => 14,
            $enum::NormalizedForeignCall => 87,
            $enum::ExactMultiplyI64 => 88,
            $enum::ExactRemainderU64 => 89,
            $enum::WrappingSubtractI64 => 90,
            $enum::WrappingMultiplyI64 => 91,
            $enum::WrappingDivideI64 => 92,
            $enum::BitwiseOrI64 => 93,
            $enum::BitwiseNotI64 => 94,
            $enum::SaveFloatingControl => 103,
            $enum::RestoreFloatingControl => 104,
            $enum::WrappingShiftLeftI64 => 105,
            $enum::WrappingShiftRightI64 => 106,
            $enum::WrappingShiftRightU64 => 107,
            $enum::ExactShiftLeftI64 => 108,
            $enum::ExactShiftRightI64 => 109,
            $enum::ExactShiftRightU64 => 110,
            $enum::ExactDivideI64 => 112,
            $enum::ExactRemainderI64 => 113,
            $enum::SaturatingMultiply(carrier) => {
                saturating_family_tag(SaturatingOperation::Multiply, carrier)
            }
            $enum::TrappingInteger(form) => trapping_family_tag(form),
            $enum::AtomicFence => 238,
            $enum::AtomicAccess(form) => form.machine_tag(),
            $enum::PlacedScalarAccess(form) => form.machine_tag(),
            $enum::NearestIeeeFloatArithmetic(form) => form.machine_tag(),
            $enum::FloatResultPolicy(form) => form.machine_tag(),
        }
    };
}

pub(crate) const fn semantic_kind_tag(kind: MachineSemanticKind) -> u8 {
    effect_tag_arms!(MachineSemanticKind, kind)
}

pub(crate) const fn semantic_kind_payload(kind: MachineSemanticKind) -> Option<u8> {
    match kind {
        MachineSemanticKind::PlacedScalarAccess(form) => Some(form.machine_payload()),
        MachineSemanticKind::NearestIeeeFloatArithmetic(form) => Some(form.machine_payload()),
        MachineSemanticKind::FloatResultPolicy(form) => Some(form.machine_payload()),
        _ => None,
    }
}

pub const fn alternative_family_tag(family: MachineAlternativeFamily) -> u8 {
    effect_tag_arms!(MachineAlternativeFamily, family)
}

pub(crate) const fn alternative_family_payload(family: MachineAlternativeFamily) -> Option<u8> {
    match family {
        MachineAlternativeFamily::PlacedScalarAccess(form) => Some(form.machine_payload()),
        MachineAlternativeFamily::NearestIeeeFloatArithmetic(form) => Some(form.machine_payload()),
        MachineAlternativeFamily::FloatResultPolicy(form) => Some(form.machine_payload()),
        _ => None,
    }
}

fn encode_len<Sink: MachineIdentityBytes + ?Sized>(bytes: &mut Sink, value: usize) {
    bytes.identity_bytes(
        &u64::try_from(value)
            .expect("machine-effect catalog length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests;

/// The one machine-semantic and alternative-family tag of each saturating
/// operation and carrier, shared by every identity table that encodes the
/// family enums. The forms that existed before the family was widened keep
/// their tags (u64 subtract 54, u64 add 55, i32 add/subtract/divide 60, 61,
/// 62); every other carrier takes its ordinal above a per-operation base
/// (add 63, subtract 71, divide 79, remainder 95, multiply 114). Tags are
/// appended, never reused.
/// The canonical identity encoding of a [`crate::MachineAlternativeKey`]: the
/// family tag from [`alternative_family_tag`] followed by the little-endian
/// variant number. Machine-code fragment, layout, text-section, relaxation and
/// encoding identities all hash exactly these bytes.
pub fn encode_machine_alternative_key_identity<Sink: MachineIdentityBytes + ?Sized>(
    bytes: &mut Sink,
    key: crate::MachineAlternativeKey,
) {
    bytes.identity_bytes(&[alternative_family_tag(key.family)]);
    if let Some(payload) = alternative_family_payload(key.family) {
        bytes.identity_bytes(&[payload]);
    }
    bytes.identity_bytes(&key.variant.to_le_bytes());
}

pub const fn saturating_family_tag(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
) -> u8 {
    match (operation, carrier) {
        (SaturatingOperation::Subtract, SaturatingCarrier::U64) => 54,
        (SaturatingOperation::Add, SaturatingCarrier::U64) => 55,
        (SaturatingOperation::Add, SaturatingCarrier::I32) => 60,
        (SaturatingOperation::Subtract, SaturatingCarrier::I32) => 61,
        (SaturatingOperation::Divide, SaturatingCarrier::I32) => 62,
        (SaturatingOperation::Add, carrier) => 63 + carrier.ordinal(),
        (SaturatingOperation::Subtract, carrier) => 71 + carrier.ordinal(),
        (SaturatingOperation::Divide, carrier) => 79 + carrier.ordinal(),
        (SaturatingOperation::Remainder, carrier) => 95 + carrier.ordinal(),
        (SaturatingOperation::Multiply, carrier) => 114 + carrier.ordinal(),
    }
}

/// The one machine-semantic and alternative-family tag of each Trapping
/// form: 122 plus the form's dense ordinal (122 through 193), above every
/// fixed and saturating tag. Forms are appended, never renumbered.
pub const fn trapping_family_tag(form: crate::TrappingForm) -> u8 {
    122 + form.ordinal()
}
