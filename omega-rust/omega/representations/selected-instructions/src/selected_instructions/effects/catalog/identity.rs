use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
use target::{Architecture, NativeTarget, ObjectFormat};

use crate::{
    MachineAlternativeApplicability, MachineAlternativeFamily, MachineBarrier,
    MachineEffectCatalog, MachineEffectCatalogIdentity, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineSemanticKind, MachineSizeKnowledge,
};

pub fn machine_effect_catalog_identity(
    catalog: &MachineEffectCatalog,
) -> MachineEffectCatalogIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-machine-effect-catalog.v22\0");
    encode_target(&mut bytes, catalog.target);
    bytes.extend_from_slice(&catalog.register_constraints.bytes());
    for key in [
        catalog.selected_keys.hosted_read_byte,
        catalog.selected_keys.hosted_write_byte_i32,
        catalog.selected_keys.store,
        catalog.selected_keys.address_offset,
        catalog.selected_keys.load64,
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
    ] {
        bytes.push(u8::from(key.is_some()));
    }
    encode_len(&mut bytes, catalog.selected_keys.call_unit.len());
    encode_len(&mut bytes, catalog.selected_keys.call_unit_mixed.len());
    encode_len(&mut bytes, catalog.selected_keys.call_i64.len());
    encode_len(&mut bytes, catalog.selected_keys.call_aggregate.len());
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
            crate::MachineMemoryEffect::NoneV1 => 0,
            crate::MachineMemoryEffect::ReadPointerV1 => 1,
            crate::MachineMemoryEffect::HostedReadByteV1 => 5,
            crate::MachineMemoryEffect::HostedWriteByteV1 => 3,
            crate::MachineMemoryEffect::WriteFrameStorageV1 => 2,
            crate::MachineMemoryEffect::WritePointerV1 => 4,
        });
        bytes.push(match declaration.trap {
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
        }
        bytes.push(match declaration.cleanup {
            crate::MachineCleanupEffect::NoneV1 => 0,
        });
        encode_len(&mut bytes, declaration.alternatives.len());
        for alternative in &declaration.alternatives {
            bytes.push(alternative_family_tag(alternative.key.family));
            bytes.extend_from_slice(&alternative.key.variant.to_le_bytes());
            match alternative.applicability {
                MachineAlternativeApplicability::Always => bytes.push(0),
                MachineAlternativeApplicability::ResultAliasesOperand { result, operand } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&result.to_le_bytes());
                    bytes.extend_from_slice(&operand.to_le_bytes());
                }
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result,
                    aliased_operand,
                    distinct_operand,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&result.to_le_bytes());
                    bytes.extend_from_slice(&aliased_operand.to_le_bytes());
                    bytes.extend_from_slice(&distinct_operand.to_le_bytes());
                }
                MachineAlternativeApplicability::ResultAliasesOperands {
                    result,
                    left,
                    right,
                } => {
                    bytes.push(3);
                    bytes.extend_from_slice(&result.to_le_bytes());
                    bytes.extend_from_slice(&left.to_le_bytes());
                    bytes.extend_from_slice(&right.to_le_bytes());
                }
                MachineAlternativeApplicability::ResultDistinctFromOperands {
                    result,
                    left,
                    right,
                } => {
                    bytes.push(4);
                    bytes.extend_from_slice(&result.to_le_bytes());
                    bytes.extend_from_slice(&left.to_le_bytes());
                    bytes.extend_from_slice(&right.to_le_bytes());
                }
                MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
                    left,
                    right,
                    excluded_view,
                } => {
                    bytes.push(5);
                    bytes.extend_from_slice(&left.to_le_bytes());
                    bytes.extend_from_slice(&right.to_le_bytes());
                    bytes.extend_from_slice(&excluded_view.0.to_le_bytes());
                }
            }
            match alternative.size {
                MachineSizeKnowledge::ExactBytes(count) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&count.to_le_bytes());
                }
                MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes,
                    maximum_bytes,
                } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&minimum_bytes.to_le_bytes());
                    match maximum_bytes {
                        None => bytes.push(0),
                        Some(maximum) => {
                            bytes.push(1);
                            bytes.extend_from_slice(&maximum.to_le_bytes());
                        }
                    }
                }
            }
            bytes.push(match alternative.latency {
                MachineLatencyKnowledge::StableBaselineUnavailable => 0,
            });
            encode_encoded_effects(&mut bytes, &alternative.encoded);
        }
    }
    MachineEffectCatalogIdentity::from_canonical_bytes(&bytes)
}

fn encode_encoded_effects(bytes: &mut Vec<u8>, effects: &MachineEncodedEffects) {
    encode_u16s(bytes, &effects.external_operand_reads);
    encode_u16s(bytes, &effects.external_operand_writes);
    encode_units(bytes, &effects.implicit_unit_uses);
    encode_units(bytes, &effects.implicit_unit_defs);
    encode_units(bytes, &effects.implicit_unit_clobbers);
    match effects.memory {
        MachineEncodedMemoryEffect::HostedReadByteV1 { stack_pointer } => {
            bytes.push(8);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
        }
        MachineEncodedMemoryEffect::HostedWriteByteV1 { stack_pointer } => {
            bytes.push(6);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
        }
        MachineEncodedMemoryEffect::NoneV1 => bytes.push(0),
        MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand } => {
            bytes.push(7);
            bytes.extend_from_slice(&pointer_operand.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand,
            byte_count,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&pointer_operand.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
            pointer_operand,
            index_operand,
            byte_count,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&pointer_operand.to_le_bytes());
            bytes.extend_from_slice(&index_operand.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WriteFrameStorageV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
        MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        MachineEncodedStackEffect::UnchangedV1 => bytes.push(0),
        MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer,
            return_address_byte_count,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&return_address_byte_count.to_le_bytes());
        }
    }
    bytes.push(match effects.trap {
        MachineEncodedTrapBehavior::NeverV1 => 0,
        MachineEncodedTrapBehavior::HostedExitReturnedV1 => 3,
        MachineEncodedTrapBehavior::HostedReadFailureV1 => 4,
        MachineEncodedTrapBehavior::HostedWriteFailureV1 => 2,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    match effects.control {
        MachineEncodedControlEffect::HostedExitOrTrapV1 => bytes.push(7),
        MachineEncodedControlEffect::HostedReadReturnOrTrapV1 => bytes.push(8),
        MachineEncodedControlEffect::HostedWriteReturnOrTrapV1 => bytes.push(6),
        MachineEncodedControlEffect::FallThroughV1 => bytes.push(0),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1 => bytes.push(1),
        MachineEncodedControlEffect::ReturnFromActivationStackV1 => bytes.push(2),
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            bytes.push(3);
            bytes.extend_from_slice(&target.0.to_le_bytes());
        }
        MachineEncodedControlEffect::DirectRelativeCallV1 => bytes.push(4),
        MachineEncodedControlEffect::UnconditionalRelativeBranchV1 => bytes.push(5),
    }
}

fn encode_u16s(bytes: &mut Vec<u8>, values: &[u16]) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_units(bytes: &mut Vec<u8>, units: &[register_model::RegisterUnitId]) {
    encode_len(bytes, units.len());
    for unit in units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
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
        MachineSemanticKind::Load64 => 16,
        MachineSemanticKind::Load8 => 33,
        MachineSemanticKind::Load16 => 34,
        MachineSemanticKind::Load32 => 30,
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
        MachineSemanticKind::ExactAddI64Immediate => 4,
        MachineSemanticKind::ExactSubtractI64 => 5,
        MachineSemanticKind::ConditionalBranchNonZero => 6,
        MachineSemanticKind::ReturnI64 => 7,
        MachineSemanticKind::ExactSubtractI64Immediate => 8,
        MachineSemanticKind::ReturnUnit => 9,
        MachineSemanticKind::CompareI64 => 10,
        MachineSemanticKind::ConditionalBranchU64LessThan => 11,
        MachineSemanticKind::ConditionalBranchI64LessThan => 12,
        MachineSemanticKind::CallI64 => 13,
        MachineSemanticKind::Jump => 14,
    }
}

pub(crate) const fn alternative_family_tag(family: MachineAlternativeFamily) -> u8 {
    match family {
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
        MachineAlternativeFamily::Load64 => 16,
        MachineAlternativeFamily::Load8 => 33,
        MachineAlternativeFamily::Load16 => 34,
        MachineAlternativeFamily::Load32 => 30,
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
        MachineAlternativeFamily::ExactAddI64Immediate => 4,
        MachineAlternativeFamily::ExactSubtractI64 => 5,
        MachineAlternativeFamily::ConditionalBranchNonZero => 6,
        MachineAlternativeFamily::ReturnI64 => 7,
        MachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        MachineAlternativeFamily::ReturnUnit => 9,
        MachineAlternativeFamily::CompareI64 => 10,
        MachineAlternativeFamily::ConditionalBranchU64LessThan => 11,
        MachineAlternativeFamily::ConditionalBranchI64LessThan => 12,
        MachineAlternativeFamily::CallI64 => 13,
        MachineAlternativeFamily::Jump => 14,
    }
}

fn encode_len(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("machine-effect catalog length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests;
