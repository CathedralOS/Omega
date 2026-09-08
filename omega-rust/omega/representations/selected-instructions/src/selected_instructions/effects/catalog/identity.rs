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
    bytes.extend_from_slice(b"omega.terminal-machine-effect-catalog.v18\0");
    encode_target(&mut bytes, catalog.target);
    bytes.extend_from_slice(&catalog.register_constraints.bytes());
    for key in [
        catalog.selected_keys.linux_write_byte_i32,
        catalog.selected_keys.store,
        catalog.selected_keys.address_offset,
        catalog.selected_keys.load64,
        catalog.selected_keys.load8_indexed,
        catalog.selected_keys.store64,
        catalog.selected_keys.frame_address,
    ] {
        bytes.push(u8::from(key.is_some()));
    }
    encode_len(&mut bytes, catalog.selected_keys.call_unit.len());
    encode_len(&mut bytes, catalog.selected_keys.call_i64.len());
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
            crate::MachineMemoryEffect::LinuxWriteByteV1 => 3,
            crate::MachineMemoryEffect::WriteFrameStorageV1 => 2,
            crate::MachineMemoryEffect::WritePointerV1 => 4,
        });
        bytes.push(match declaration.trap {
            crate::MachineTrapBehavior::NeverV1 => 0,
            crate::MachineTrapBehavior::LinuxWriteFailureV1 => 2,
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
        MachineEncodedMemoryEffect::LinuxWriteByteV1 { stack_pointer } => {
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
        MachineEncodedTrapBehavior::LinuxWriteFailureV1 => 2,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    match effects.control {
        MachineEncodedControlEffect::LinuxWriteReturnOrTrapV1 => bytes.push(6),
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
        MachineSemanticKind::CompareI64Zero => 0,
        MachineSemanticKind::MaterializeI64 => 1,
        MachineSemanticKind::CopyI64 => 2,
        MachineSemanticKind::ZeroExtendU8 => 15,
        MachineSemanticKind::ZeroExtendU32 => 20,
        MachineSemanticKind::Load64 => 16,
        MachineSemanticKind::LinuxWriteByteI32 => 23,
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
        MachineAlternativeFamily::CompareI64Zero => 0,
        MachineAlternativeFamily::MaterializeI64 => 1,
        MachineAlternativeFamily::CopyI64 => 2,
        MachineAlternativeFamily::ZeroExtendU8 => 15,
        MachineAlternativeFamily::ZeroExtendU32 => 20,
        MachineAlternativeFamily::Load64 => 16,
        MachineAlternativeFamily::LinuxWriteByteI32 => 23,
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
mod tests {
    use register_model::{
        RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    };
    use target::NativeTarget;

    use super::*;
    use crate::{
        MachineAlternative, MachineAlternativeKey, MachineCallEffect, MachineCleanupEffect,
        MachineEffectDeclaration, MachineLatencyKnowledge, MachineMemoryEffect,
        MachineSizeKnowledge, MachineTrapBehavior, SelectedConstraintKeys,
    };

    const fn instruction(variant: u32) -> RegisterConstraintKey {
        RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant,
        }
    }

    fn keys() -> SelectedConstraintKeys {
        SelectedConstraintKeys {
            linux_write_byte_i32: Some(instruction(24)),
            load64: Some(instruction(20)),
            load8_indexed: Some(instruction(23)),
            store: Some(instruction(25)),
            address_offset: Some(instruction(26)),
            store64: Some(instruction(21)),
            frame_address: Some(instruction(22)),
            call_unit: vec![RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 2,
            }],
            call_i64: vec![RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 3,
            }],
            materialize_i64: instruction(0),
            copy_i64: instruction(1),
            add_i64: instruction(2),
            subtract_i64: instruction(4),
            add_i64_immediate: instruction(3),
            subtract_i64_immediate: instruction(8),
            compare_i64_zero: instruction(5),
            compare_i64: instruction(15),
            conditional_branch: instruction(6),
            jump: instruction(16),
            return_i64: RegisterConstraintKey {
                family: RegisterConstraintFamily::Return,
                variant: 0,
            },
            return_unit: RegisterConstraintKey {
                family: RegisterConstraintFamily::Return,
                variant: 1,
            },
        }
    }

    fn declaration(semantic: MachineSemanticKind) -> MachineEffectDeclaration {
        let keys = keys();
        let constraint = keys
            .for_semantic(semantic)
            .or_else(|| {
                (matches!(
                    semantic,
                    MachineSemanticKind::CallI64 | MachineSemanticKind::CallUnit
                ))
                .then_some(keys.call_i64[0])
            })
            .expect("test catalog declares every semantic constraint");
        MachineEffectDeclaration {
            semantic,
            constraint,
            memory: if semantic == MachineSemanticKind::LinuxWriteByteI32 {
                MachineMemoryEffect::LinuxWriteByteV1
            } else {
                MachineMemoryEffect::NoneV1
            },
            trap: if semantic == MachineSemanticKind::LinuxWriteByteI32 {
                MachineTrapBehavior::LinuxWriteFailureV1
            } else {
                MachineTrapBehavior::NeverV1
            },
            barrier: if semantic == MachineSemanticKind::LinuxWriteByteI32 {
                MachineBarrier::ExternalEffect
            } else if matches!(
                semantic,
                MachineSemanticKind::ConditionalBranchNonZero
                    | MachineSemanticKind::ConditionalBranchU64LessThan
                    | MachineSemanticKind::ConditionalBranchI64LessThan
                    | MachineSemanticKind::ReturnI64
                    | MachineSemanticKind::ReturnUnit
            ) {
                MachineBarrier::ControlFlow
            } else if matches!(
                semantic,
                MachineSemanticKind::CallI64 | MachineSemanticKind::CallUnit
            ) {
                MachineBarrier::Call
            } else {
                MachineBarrier::None
            },
            call: if matches!(
                semantic,
                MachineSemanticKind::CallI64 | MachineSemanticKind::CallUnit
            ) {
                MachineCallEffect::DirectInternalNormalReturnV1 {
                    pre_call_stack_alignment: 16,
                }
            } else {
                MachineCallEffect::NoneV1
            },
            cleanup: MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(4),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: if semantic == MachineSemanticKind::LinuxWriteByteI32 {
                    let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![]);
                    encoded.memory = MachineEncodedMemoryEffect::LinuxWriteByteV1 {
                        stack_pointer: register_model::RegisterViewId(7),
                    };
                    encoded.trap = MachineEncodedTrapBehavior::LinuxWriteFailureV1;
                    encoded.control = MachineEncodedControlEffect::LinuxWriteReturnOrTrapV1;
                    encoded
                } else {
                    MachineEncodedEffects::fallthrough_v1(vec![], vec![])
                },
            }],
        }
    }

    fn catalog() -> MachineEffectCatalog {
        MachineEffectCatalog {
            target: NativeTarget::linux_x64(),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([1; 32]),
            selected_keys: keys(),
            declarations: MachineSemanticKind::ALL
                .into_iter()
                .map(declaration)
                .collect(),
        }
    }

    #[test]
    fn identity_binds_memory_call_and_subtraction_alternatives() {
        let source = catalog();
        let baseline = machine_effect_catalog_identity(&source);
        assert_eq!(baseline, machine_effect_catalog_identity(&source));

        let mut changed = source.clone();
        changed.target = NativeTarget::linux_arm64();
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
        let mut changed = source.clone();
        changed.register_constraints = RegisterConstraintCatalogIdentity::from_bytes([2; 32]);
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
        let mut changed = source.clone();
        changed.selected_keys.subtract_i64 = instruction(99);
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
        let mut changed = source.clone();
        changed.selected_keys.call_unit.clear();
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
        let mut changed = source.clone();
        let call = changed
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::CallUnit)
            .unwrap();
        call.call = MachineCallEffect::DirectInternalNormalReturnV1 {
            pre_call_stack_alignment: 32,
        };
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
        let mut changed = source.clone();
        let subtract = changed
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::ExactSubtractI64)
            .unwrap();
        subtract.alternatives[0].applicability =
            MachineAlternativeApplicability::ResultAliasesOperand {
                result: 2,
                operand: 0,
            };
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
        let mut changed = source;
        changed.declarations[0].barrier = MachineBarrier::ControlFlow;
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
    }

    #[test]
    fn identity_distinguishes_call_arity_order_and_role_boundaries() {
        let mut source = catalog();
        source.selected_keys.call_i64.push(RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 4,
        });
        let baseline = machine_effect_catalog_identity(&source);
        let mut reordered = source.clone();
        reordered.selected_keys.call_i64.swap(0, 1);
        assert_ne!(baseline, machine_effect_catalog_identity(&reordered));
        let mut relabeled = source.clone();
        let structural_key = relabeled.selected_keys.call_unit.remove(0);
        relabeled.selected_keys.call_i64.insert(0, structural_key);
        assert_eq!(
            source.selected_keys.in_identity_order(),
            relabeled.selected_keys.in_identity_order()
        );
        assert_ne!(baseline, machine_effect_catalog_identity(&relabeled));
    }
}
