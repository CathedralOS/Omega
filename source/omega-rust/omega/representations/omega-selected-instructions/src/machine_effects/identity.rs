use omega_register_model::{RegisterConstraintFamily, RegisterConstraintKey};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

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
    bytes.extend_from_slice(b"omega.terminal-machine-effect-catalog.v5\0");
    encode_target(&mut bytes, catalog.target);
    bytes.extend_from_slice(&catalog.register_constraints.bytes());
    let selected_keys = catalog.selected_keys.in_identity_order();
    encode_len(&mut bytes, selected_keys.len());
    for key in selected_keys {
        encode_constraint_key(&mut bytes, key);
    }
    match catalog.structural_unit_call {
        None => bytes.push(0),
        Some(declaration) => {
            bytes.push(1);
            encode_constraint_key(&mut bytes, declaration.constraint);
            match declaration.memory {
                crate::StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
                    root_byte_count,
                    copy_stack_byte_offsets,
                } => {
                    bytes.push(0);
                    bytes.extend_from_slice(&root_byte_count.to_le_bytes());
                    for offset in copy_stack_byte_offsets {
                        bytes.extend_from_slice(&offset.to_le_bytes());
                    }
                }
            }
            match declaration.frame {
                crate::StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
                    frame_byte_count,
                    shadow_byte_count,
                    pre_call_stack_alignment,
                } => {
                    bytes.push(0);
                    bytes.extend_from_slice(&frame_byte_count.to_le_bytes());
                    bytes.extend_from_slice(&shadow_byte_count.to_le_bytes());
                    bytes.extend_from_slice(&pre_call_stack_alignment.to_le_bytes());
                }
            }
            bytes.push(match declaration.trap {
                crate::MachineTrapBehavior::NeverV1 => 0,
                crate::MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
            });
            bytes.push(match declaration.barrier {
                crate::StructuralUnitCallBarrier::CallV1 => 0,
            });
            bytes.push(match declaration.call {
                crate::StructuralUnitCallEffect::DirectInternalUnitV1 => 0,
            });
            bytes.push(match declaration.cleanup {
                crate::MachineCleanupEffect::NoneV1 => 0,
            });
        }
    }
    encode_len(&mut bytes, catalog.declarations.len());
    for declaration in &catalog.declarations {
        bytes.push(semantic_kind_tag(declaration.semantic));
        encode_constraint_key(&mut bytes, declaration.constraint);
        // The v1 semantic vocabulary is deliberately closed to effect-free
        // scalar work plus explicit control-flow barriers. Keep these matches
        // exhaustive so adding a vocabulary variant cannot silently collide
        // with an older catalog identity.
        bytes.push(match declaration.memory {
            crate::MachineMemoryEffect::NoneV1 => 0,
        });
        bytes.push(match declaration.trap {
            crate::MachineTrapBehavior::NeverV1 => 0,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
        });
        bytes.push(match declaration.barrier {
            MachineBarrier::None => 0,
            MachineBarrier::ControlFlow => 1,
        });
        bytes.push(match declaration.call {
            crate::MachineCallEffect::NoneV1 => 0,
        });
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
        MachineEncodedMemoryEffect::NoneV1 => bytes.push(0),
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(1);
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
    }
    bytes.push(match effects.trap {
        MachineEncodedTrapBehavior::NeverV1 => 0,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    match effects.control {
        MachineEncodedControlEffect::FallThroughV1 => bytes.push(0),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1 => bytes.push(1),
        MachineEncodedControlEffect::ReturnFromActivationStackV1 => bytes.push(2),
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            bytes.push(3);
            bytes.extend_from_slice(&target.0.to_le_bytes());
        }
    }
}

fn encode_u16s(bytes: &mut Vec<u8>, values: &[u16]) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_units(bytes: &mut Vec<u8>, units: &[omega_register_model::RegisterUnitId]) {
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
        MachineSemanticKind::ExactAddI64 => 3,
        MachineSemanticKind::ExactAddI64Immediate => 4,
        MachineSemanticKind::ExactSubtractI64 => 5,
        MachineSemanticKind::ConditionalBranchNonZero => 6,
        MachineSemanticKind::ReturnI64 => 7,
        MachineSemanticKind::ExactSubtractI64Immediate => 8,
        MachineSemanticKind::ReturnUnit => 9,
        MachineSemanticKind::CompareI64 => 10,
    }
}

pub(crate) const fn alternative_family_tag(family: MachineAlternativeFamily) -> u8 {
    match family {
        MachineAlternativeFamily::CompareI64Zero => 0,
        MachineAlternativeFamily::MaterializeI64 => 1,
        MachineAlternativeFamily::CopyI64 => 2,
        MachineAlternativeFamily::ExactAddI64 => 3,
        MachineAlternativeFamily::ExactAddI64Immediate => 4,
        MachineAlternativeFamily::ExactSubtractI64 => 5,
        MachineAlternativeFamily::ConditionalBranchNonZero => 6,
        MachineAlternativeFamily::ReturnI64 => 7,
        MachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        MachineAlternativeFamily::ReturnUnit => 9,
        MachineAlternativeFamily::CompareI64 => 10,
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
    use omega_register_model::{
        RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    };
    use omega_target::NativeTarget;

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
            structural_unit_call: Some(RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 2,
            }),
            materialize_i64: instruction(0),
            copy_i64: instruction(1),
            add_i64: instruction(2),
            subtract_i64: instruction(4),
            add_i64_immediate: instruction(3),
            subtract_i64_immediate: instruction(8),
            compare_i64_zero: instruction(5),
            compare_i64: instruction(15),
            conditional_branch: instruction(6),
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
        let constraint = keys.for_semantic(semantic);
        MachineEffectDeclaration {
            semantic,
            constraint,
            memory: MachineMemoryEffect::NoneV1,
            trap: MachineTrapBehavior::NeverV1,
            barrier: if matches!(
                semantic,
                MachineSemanticKind::ConditionalBranchNonZero
                    | MachineSemanticKind::ReturnI64
                    | MachineSemanticKind::ReturnUnit
            ) {
                MachineBarrier::ControlFlow
            } else {
                MachineBarrier::None
            },
            call: MachineCallEffect::NoneV1,
            cleanup: MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(4),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![]),
            }],
        }
    }

    fn catalog() -> MachineEffectCatalog {
        MachineEffectCatalog {
            target: NativeTarget::linux_x64(),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([1; 32]),
            selected_keys: keys(),
            structural_unit_call: Some(crate::StructuralUnitCallEffectDeclaration {
                constraint: keys().structural_unit_call.unwrap(),
                memory: crate::StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
                    root_byte_count: 16,
                    copy_stack_byte_offsets: [32, 48],
                },
                frame: crate::StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
                    frame_byte_count: 72,
                    shadow_byte_count: 32,
                    pre_call_stack_alignment: 16,
                },
                trap: MachineTrapBehavior::MayArchitecturalFaultV1,
                barrier: crate::StructuralUnitCallBarrier::CallV1,
                call: crate::StructuralUnitCallEffect::DirectInternalUnitV1,
                cleanup: MachineCleanupEffect::NoneV1,
            }),
            declarations: MachineSemanticKind::ALL
                .into_iter()
                .map(declaration)
                .collect(),
        }
    }

    #[test]
    fn identity_binds_structural_call_and_subtraction_alternatives() {
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
        changed.selected_keys.structural_unit_call = None;
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
        let mut changed = source.clone();
        let Some(structural) = changed.structural_unit_call.as_mut() else {
            panic!("fixture owns the structural call declaration");
        };
        let crate::StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count, ..
        } = &mut structural.frame;
        *frame_byte_count = 64;
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
}
