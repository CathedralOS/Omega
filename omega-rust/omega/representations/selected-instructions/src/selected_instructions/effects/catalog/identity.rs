use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
use target::{Architecture, NativeTarget, ObjectFormat};

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

pub fn machine_effect_catalog_identity(
    catalog: &MachineEffectCatalog,
) -> MachineEffectCatalogIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-machine-effect-catalog.v28\0");
    encode_target(&mut bytes, catalog.target);
    bytes.extend_from_slice(&catalog.register_constraints.bytes());
    for key in [
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
        catalog.selected_keys.float32_to_bits,
        catalog.selected_keys.float64_to_bits,
        catalog.selected_keys.bits_to_float32,
        catalog.selected_keys.bits_to_float64,
        catalog.selected_keys.save_floating_control,
        catalog.selected_keys.restore_floating_control,
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
        encode_constraint_key(&mut bytes, declaration.constraint);
        // Keep effect matches exhaustive so additions cannot silently collide
        // with an older catalog identity.
        bytes.push(match declaration.memory {
            crate::MachineMemoryEffect::CopyBytesV1 => 6,
            crate::MachineMemoryEffect::NoneV1 => 0,
            crate::MachineMemoryEffect::ReadPointerV1 => 1,
            crate::MachineMemoryEffect::HostedReadByteV1 => 5,
            crate::MachineMemoryEffect::HostedWriteByteV1 => 3,
            crate::MachineMemoryEffect::WriteFrameStorageV1 => 2,
            crate::MachineMemoryEffect::ReadFrameStorageV1 => 7,
            crate::MachineMemoryEffect::WritePointerV1 => 4,
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
            crate::MachineCallEffect::DirectInternalNormalReturnV1 {
                pre_call_stack_alignment,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&pre_call_stack_alignment.to_le_bytes());
            }
            crate::MachineCallEffect::DirectExternalNormalReturnV1 {
                pre_call_stack_alignment,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&pre_call_stack_alignment.to_le_bytes());
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
        MachineEncodedMemoryEffect::NoneV1 => bytes.identity_bytes(&[0]),
        MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand } => {
            bytes.identity_bytes(&[7]);
            bytes.identity_bytes(&pointer_operand.to_le_bytes());
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

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_size)
            .expect("target pointer size fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_alignment)
            .expect("target pointer alignment fits u64")
            .to_le_bytes(),
    );
}

fn encode_constraint_key(bytes: &mut Vec<u8>, key: RegisterConstraintKey) {
    bytes.push(match key.family {
        RegisterConstraintFamily::Call => 0,
        RegisterConstraintFamily::Return => 1,
        RegisterConstraintFamily::SystemCall => 2,
        RegisterConstraintFamily::InlineAssembly => 3,
        RegisterConstraintFamily::Instruction => 4,
    });
    bytes.extend_from_slice(&key.variant.to_le_bytes());
}

pub(crate) const fn semantic_kind_tag(kind: MachineSemanticKind) -> u8 {
    match kind {
        MachineSemanticKind::CopyBytes => 59,
        MachineSemanticKind::CallAggregate => 35,
        MachineSemanticKind::ReturnAggregate => 36,
        MachineSemanticKind::CompareI64Zero => 0,
        MachineSemanticKind::MaterializeI64 => 1,
        MachineSemanticKind::CopyI64 => 2,
        MachineSemanticKind::Float32ToBits => 26,
        MachineSemanticKind::Float64ToBits => 27,
        MachineSemanticKind::BitsToFloat32 => 28,
        MachineSemanticKind::BitsToFloat64 => 29,
        MachineSemanticKind::ZeroExtendU8 => 15,
        MachineSemanticKind::ZeroExtendU32 => 20,
        MachineSemanticKind::ZeroExtendU16 => 37,
        MachineSemanticKind::SignExtendI8 => 38,
        MachineSemanticKind::SignExtendI16 => 39,
        MachineSemanticKind::SignExtendI32 => 40,
        MachineSemanticKind::MaterializeBooleanEqual => 41,
        MachineSemanticKind::MaterializeBooleanU64LessThan => 42,
        MachineSemanticKind::MaterializeBooleanI64LessThan => 43,
        MachineSemanticKind::MaterializeBooleanU64LessOrEqual => 44,
        MachineSemanticKind::MaterializeBooleanI64LessOrEqual => 45,
        MachineSemanticKind::Load64 => 16,
        MachineSemanticKind::LoadPacked3 => 46,
        MachineSemanticKind::LoadPacked5 => 47,
        MachineSemanticKind::LoadPacked6 => 48,
        MachineSemanticKind::LoadPacked7 => 49,
        MachineSemanticKind::StorePacked => 50,
        MachineSemanticKind::Load8 => 33,
        MachineSemanticKind::Load16 => 34,
        MachineSemanticKind::Load32 => 30,
        MachineSemanticKind::Crash => 111,
        MachineSemanticKind::HostedExitProcessI32 => 31,
        MachineSemanticKind::HostedReadByte => 32,
        MachineSemanticKind::HostedWriteByteI32 => 23,
        MachineSemanticKind::Store => 24,
        MachineSemanticKind::AddressOffset => 25,
        MachineSemanticKind::ByteViewAddress => 22,
        MachineSemanticKind::Load8Indexed => 21,
        MachineSemanticKind::Store64 => 17,
        MachineSemanticKind::FrameAddress => 18,
        MachineSemanticKind::CallUnit => 19,
        MachineSemanticKind::ExactAddI64 => 3,
        MachineSemanticKind::BitwiseAndI64 => 51,
        MachineSemanticKind::BitwiseXorI64 => 52,
        MachineSemanticKind::ExactDivideU64 => 56,
        MachineSemanticKind::WrappingRemainderI64 => 57,
        MachineSemanticKind::WrappingAddI64 => 58,
        MachineSemanticKind::SaturatingAdd(carrier) => {
            saturating_family_tag(SaturatingOperation::Add, carrier)
        }
        MachineSemanticKind::SaturatingSubtract(carrier) => {
            saturating_family_tag(SaturatingOperation::Subtract, carrier)
        }
        MachineSemanticKind::SaturatingDivide(carrier) => {
            saturating_family_tag(SaturatingOperation::Divide, carrier)
        }
        MachineSemanticKind::SaturatingRemainder(carrier) => {
            saturating_family_tag(SaturatingOperation::Remainder, carrier)
        }
        MachineSemanticKind::ExactAddI64Immediate => 4,
        MachineSemanticKind::ExactSubtractI64 => 5,
        MachineSemanticKind::ConditionalBranchNonZero => 6,
        MachineSemanticKind::ReturnScalar => 7,
        MachineSemanticKind::ExactSubtractI64Immediate => 8,
        MachineSemanticKind::ReturnUnit => 9,
        MachineSemanticKind::CompareI64 => 10,
        MachineSemanticKind::CompareI64Immediate => 53,
        MachineSemanticKind::ConditionalBranchU64LessThan => 11,
        MachineSemanticKind::ConditionalBranchI64LessThan => 12,
        MachineSemanticKind::CallScalar => 13,
        MachineSemanticKind::Jump => 14,
        MachineSemanticKind::NormalizedForeignCall => 87,
        MachineSemanticKind::ExactMultiplyI64 => 88,
        MachineSemanticKind::ExactRemainderU64 => 89,
        MachineSemanticKind::WrappingSubtractI64 => 90,
        MachineSemanticKind::WrappingMultiplyI64 => 91,
        MachineSemanticKind::WrappingDivideI64 => 92,
        MachineSemanticKind::BitwiseOrI64 => 93,
        MachineSemanticKind::BitwiseNotI64 => 94,
        MachineSemanticKind::SaveFloatingControl => 103,
        MachineSemanticKind::RestoreFloatingControl => 104,
        MachineSemanticKind::WrappingShiftLeftI64 => 105,
        MachineSemanticKind::WrappingShiftRightI64 => 106,
        MachineSemanticKind::WrappingShiftRightU64 => 107,
        MachineSemanticKind::ExactShiftLeftI64 => 108,
        MachineSemanticKind::ExactShiftRightI64 => 109,
        MachineSemanticKind::ExactShiftRightU64 => 110,
        MachineSemanticKind::ExactDivideI64 => 112,
        MachineSemanticKind::ExactRemainderI64 => 113,
        MachineSemanticKind::SaturatingMultiply(carrier) => {
            saturating_family_tag(SaturatingOperation::Multiply, carrier)
        }
        MachineSemanticKind::TrappingInteger(form) => trapping_family_tag(form),
    }
}

/// The canonical identity tag of a [`MachineAlternativeFamily`].
///
/// This table *is* the identity: the machine-effect catalog identity, the
/// selected-effect program identity, the in-memory physical-instruction
/// identity and the machine-code fragment, layout, text-section, relaxation and
/// encoding identities all write exactly this byte for an alternative family.
/// The values are deliberately not the declaration order — they were assigned
/// as families were added and renumbering any of them would invalidate every
/// artifact already keyed by it.
///
/// Consumers must call this function rather than repeat the table. A second
/// copy of eighty-one arms is how two stages that must agree drift apart
/// silently: adding a family to one copy and not the other still compiles.
pub const fn alternative_family_tag(family: MachineAlternativeFamily) -> u8 {
    match family {
        MachineAlternativeFamily::CopyBytes => 59,
        MachineAlternativeFamily::CallAggregate => 35,
        MachineAlternativeFamily::ReturnAggregate => 36,
        MachineAlternativeFamily::CompareI64Zero => 0,
        MachineAlternativeFamily::MaterializeI64 => 1,
        MachineAlternativeFamily::CopyI64 => 2,
        MachineAlternativeFamily::Float32ToBits => 26,
        MachineAlternativeFamily::Float64ToBits => 27,
        MachineAlternativeFamily::BitsToFloat32 => 28,
        MachineAlternativeFamily::BitsToFloat64 => 29,
        MachineAlternativeFamily::ZeroExtendU8 => 15,
        MachineAlternativeFamily::ZeroExtendU32 => 20,
        MachineAlternativeFamily::ZeroExtendU16 => 37,
        MachineAlternativeFamily::SignExtendI8 => 38,
        MachineAlternativeFamily::SignExtendI16 => 39,
        MachineAlternativeFamily::SignExtendI32 => 40,
        MachineAlternativeFamily::MaterializeBooleanEqual => 41,
        MachineAlternativeFamily::MaterializeBooleanU64LessThan => 42,
        MachineAlternativeFamily::MaterializeBooleanI64LessThan => 43,
        MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual => 44,
        MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual => 45,
        MachineAlternativeFamily::Load64 => 16,
        MachineAlternativeFamily::LoadPacked3 => 46,
        MachineAlternativeFamily::LoadPacked5 => 47,
        MachineAlternativeFamily::LoadPacked6 => 48,
        MachineAlternativeFamily::LoadPacked7 => 49,
        MachineAlternativeFamily::StorePacked => 50,
        MachineAlternativeFamily::Load8 => 33,
        MachineAlternativeFamily::Load16 => 34,
        MachineAlternativeFamily::Load32 => 30,
        MachineAlternativeFamily::Crash => 111,
        MachineAlternativeFamily::HostedExitProcessI32 => 31,
        MachineAlternativeFamily::HostedReadByte => 32,
        MachineAlternativeFamily::HostedWriteByteI32 => 23,
        MachineAlternativeFamily::Store => 24,
        MachineAlternativeFamily::AddressOffset => 25,
        MachineAlternativeFamily::ByteViewAddress => 22,
        MachineAlternativeFamily::Load8Indexed => 21,
        MachineAlternativeFamily::Store64 => 17,
        MachineAlternativeFamily::FrameAddress => 18,
        MachineAlternativeFamily::CallUnit => 19,
        MachineAlternativeFamily::ExactAddI64 => 3,
        MachineAlternativeFamily::BitwiseAndI64 => 51,
        MachineAlternativeFamily::BitwiseXorI64 => 52,
        MachineAlternativeFamily::ExactDivideU64 => 56,
        MachineAlternativeFamily::WrappingRemainderI64 => 57,
        MachineAlternativeFamily::WrappingAddI64 => 58,
        MachineAlternativeFamily::SaturatingAdd(carrier) => {
            saturating_family_tag(SaturatingOperation::Add, carrier)
        }
        MachineAlternativeFamily::SaturatingSubtract(carrier) => {
            saturating_family_tag(SaturatingOperation::Subtract, carrier)
        }
        MachineAlternativeFamily::SaturatingDivide(carrier) => {
            saturating_family_tag(SaturatingOperation::Divide, carrier)
        }
        MachineAlternativeFamily::SaturatingRemainder(carrier) => {
            saturating_family_tag(SaturatingOperation::Remainder, carrier)
        }
        MachineAlternativeFamily::ExactAddI64Immediate => 4,
        MachineAlternativeFamily::ExactSubtractI64 => 5,
        MachineAlternativeFamily::ConditionalBranchNonZero => 6,
        MachineAlternativeFamily::ReturnScalar => 7,
        MachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        MachineAlternativeFamily::ReturnUnit => 9,
        MachineAlternativeFamily::CompareI64 => 10,
        MachineAlternativeFamily::CompareI64Immediate => 53,
        MachineAlternativeFamily::ConditionalBranchU64LessThan => 11,
        MachineAlternativeFamily::ConditionalBranchI64LessThan => 12,
        MachineAlternativeFamily::CallScalar => 13,
        MachineAlternativeFamily::Jump => 14,
        MachineAlternativeFamily::NormalizedForeignCall => 87,
        MachineAlternativeFamily::ExactMultiplyI64 => 88,
        MachineAlternativeFamily::ExactRemainderU64 => 89,
        MachineAlternativeFamily::WrappingSubtractI64 => 90,
        MachineAlternativeFamily::WrappingMultiplyI64 => 91,
        MachineAlternativeFamily::WrappingDivideI64 => 92,
        MachineAlternativeFamily::BitwiseOrI64 => 93,
        MachineAlternativeFamily::BitwiseNotI64 => 94,
        MachineAlternativeFamily::SaveFloatingControl => 103,
        MachineAlternativeFamily::RestoreFloatingControl => 104,
        MachineAlternativeFamily::WrappingShiftLeftI64 => 105,
        MachineAlternativeFamily::WrappingShiftRightI64 => 106,
        MachineAlternativeFamily::WrappingShiftRightU64 => 107,
        MachineAlternativeFamily::ExactShiftLeftI64 => 108,
        MachineAlternativeFamily::ExactShiftRightI64 => 109,
        MachineAlternativeFamily::ExactShiftRightU64 => 110,
        MachineAlternativeFamily::ExactDivideI64 => 112,
        MachineAlternativeFamily::ExactRemainderI64 => 113,
        MachineAlternativeFamily::SaturatingMultiply(carrier) => {
            saturating_family_tag(SaturatingOperation::Multiply, carrier)
        }
        MachineAlternativeFamily::TrappingInteger(form) => trapping_family_tag(form),
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
