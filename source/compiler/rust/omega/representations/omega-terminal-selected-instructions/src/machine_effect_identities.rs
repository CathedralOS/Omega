use omega_register_model::{RegisterConstraintFamily, RegisterConstraintKey};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

use crate::{
    TerminalMachineAlternativeApplicability, TerminalMachineAlternativeFamily,
    TerminalMachineBarrier, TerminalMachineEffectCatalog, TerminalMachineEffectCatalogIdentity,
    TerminalMachineLatencyKnowledge, TerminalMachineSemanticKind, TerminalMachineSizeKnowledge,
};

pub fn terminal_machine_effect_catalog_identity(
    catalog: &TerminalMachineEffectCatalog,
) -> TerminalMachineEffectCatalogIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-machine-effect-catalog.v1\0");
    encode_target(&mut bytes, catalog.target);
    bytes.extend_from_slice(&catalog.register_constraints.bytes());
    for key in catalog.selected_keys.in_identity_order() {
        encode_constraint_key(&mut bytes, key);
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
            crate::TerminalMachineMemoryEffect::NoneV1 => 0,
        });
        bytes.push(match declaration.trap {
            crate::TerminalMachineTrapBehavior::NeverV1 => 0,
        });
        bytes.push(match declaration.barrier {
            TerminalMachineBarrier::None => 0,
            TerminalMachineBarrier::ControlFlow => 1,
        });
        bytes.push(match declaration.call {
            crate::TerminalMachineCallEffect::NoneV1 => 0,
        });
        bytes.push(match declaration.cleanup {
            crate::TerminalMachineCleanupEffect::NoneV1 => 0,
        });
        encode_len(&mut bytes, declaration.alternatives.len());
        for alternative in &declaration.alternatives {
            bytes.push(alternative_family_tag(alternative.key.family));
            bytes.extend_from_slice(&alternative.key.variant.to_le_bytes());
            match alternative.applicability {
                TerminalMachineAlternativeApplicability::Always => bytes.push(0),
                TerminalMachineAlternativeApplicability::ResultAliasesOperand {
                    result,
                    operand,
                } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&result.to_le_bytes());
                    bytes.extend_from_slice(&operand.to_le_bytes());
                }
                TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result,
                    aliased_operand,
                    distinct_operand,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&result.to_le_bytes());
                    bytes.extend_from_slice(&aliased_operand.to_le_bytes());
                    bytes.extend_from_slice(&distinct_operand.to_le_bytes());
                }
                TerminalMachineAlternativeApplicability::ResultAliasesOperands {
                    result,
                    left,
                    right,
                } => {
                    bytes.push(3);
                    bytes.extend_from_slice(&result.to_le_bytes());
                    bytes.extend_from_slice(&left.to_le_bytes());
                    bytes.extend_from_slice(&right.to_le_bytes());
                }
                TerminalMachineAlternativeApplicability::ResultDistinctFromOperands {
                    result,
                    left,
                    right,
                } => {
                    bytes.push(4);
                    bytes.extend_from_slice(&result.to_le_bytes());
                    bytes.extend_from_slice(&left.to_le_bytes());
                    bytes.extend_from_slice(&right.to_le_bytes());
                }
            }
            match alternative.size {
                TerminalMachineSizeKnowledge::ExactBytes(count) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&count.to_le_bytes());
                }
                TerminalMachineSizeKnowledge::EncoderResolved {
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
                TerminalMachineLatencyKnowledge::StableBaselineUnavailable => 0,
            });
        }
    }
    TerminalMachineEffectCatalogIdentity::from_canonical_bytes(&bytes)
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

pub(crate) const fn semantic_kind_tag(kind: TerminalMachineSemanticKind) -> u8 {
    match kind {
        TerminalMachineSemanticKind::CompareI64Zero => 0,
        TerminalMachineSemanticKind::MaterializeI64 => 1,
        TerminalMachineSemanticKind::CopyI64 => 2,
        TerminalMachineSemanticKind::ExactAddI64 => 3,
        TerminalMachineSemanticKind::ExactAddI64Immediate => 4,
        TerminalMachineSemanticKind::ExactSubtractI64 => 5,
        TerminalMachineSemanticKind::ConditionalBranchNonZero => 6,
        TerminalMachineSemanticKind::ReturnI64 => 7,
    }
}

pub(crate) const fn alternative_family_tag(family: TerminalMachineAlternativeFamily) -> u8 {
    match family {
        TerminalMachineAlternativeFamily::CompareI64Zero => 0,
        TerminalMachineAlternativeFamily::MaterializeI64 => 1,
        TerminalMachineAlternativeFamily::CopyI64 => 2,
        TerminalMachineAlternativeFamily::ExactAddI64 => 3,
        TerminalMachineAlternativeFamily::ExactAddI64Immediate => 4,
        TerminalMachineAlternativeFamily::ExactSubtractI64 => 5,
        TerminalMachineAlternativeFamily::ConditionalBranchNonZero => 6,
        TerminalMachineAlternativeFamily::ReturnI64 => 7,
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
        TerminalMachineAlternative, TerminalMachineAlternativeKey, TerminalMachineCallEffect,
        TerminalMachineCleanupEffect, TerminalMachineEffectDeclaration,
        TerminalMachineLatencyKnowledge, TerminalMachineMemoryEffect, TerminalMachineSizeKnowledge,
        TerminalMachineTrapBehavior, TerminalSelectedConstraintKeys,
    };

    const fn instruction(variant: u32) -> RegisterConstraintKey {
        RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant,
        }
    }

    fn keys() -> TerminalSelectedConstraintKeys {
        TerminalSelectedConstraintKeys {
            materialize_i64: instruction(0),
            copy_i64: instruction(1),
            add_i64: instruction(2),
            subtract_i64: instruction(4),
            add_i64_immediate: instruction(3),
            compare_i64_zero: instruction(5),
            conditional_branch: instruction(6),
            return_i64: RegisterConstraintKey {
                family: RegisterConstraintFamily::Return,
                variant: 0,
            },
        }
    }

    fn declaration(semantic: TerminalMachineSemanticKind) -> TerminalMachineEffectDeclaration {
        let keys = keys();
        let constraint = keys.for_semantic(semantic);
        TerminalMachineEffectDeclaration {
            semantic,
            constraint,
            memory: TerminalMachineMemoryEffect::NoneV1,
            trap: TerminalMachineTrapBehavior::NeverV1,
            barrier: if matches!(
                semantic,
                TerminalMachineSemanticKind::ConditionalBranchNonZero
                    | TerminalMachineSemanticKind::ReturnI64
            ) {
                TerminalMachineBarrier::ControlFlow
            } else {
                TerminalMachineBarrier::None
            },
            call: TerminalMachineCallEffect::NoneV1,
            cleanup: TerminalMachineCleanupEffect::NoneV1,
            alternatives: vec![TerminalMachineAlternative {
                key: TerminalMachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability: TerminalMachineAlternativeApplicability::Always,
                size: TerminalMachineSizeKnowledge::ExactBytes(4),
                latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
            }],
        }
    }

    fn catalog() -> TerminalMachineEffectCatalog {
        TerminalMachineEffectCatalog {
            target: NativeTarget::linux_x64(),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([1; 32]),
            selected_keys: keys(),
            declarations: TerminalMachineSemanticKind::ALL
                .into_iter()
                .map(declaration)
                .collect(),
        }
    }

    #[test]
    fn identity_is_deterministic_and_binds_subtraction_alternatives() {
        let source = catalog();
        let baseline = terminal_machine_effect_catalog_identity(&source);
        assert_eq!(baseline, terminal_machine_effect_catalog_identity(&source));

        let mut changed = source.clone();
        changed.target = NativeTarget::linux_arm64();
        assert_ne!(baseline, terminal_machine_effect_catalog_identity(&changed));
        let mut changed = source.clone();
        changed.register_constraints = RegisterConstraintCatalogIdentity::from_bytes([2; 32]);
        assert_ne!(baseline, terminal_machine_effect_catalog_identity(&changed));
        let mut changed = source.clone();
        changed.selected_keys.subtract_i64 = instruction(99);
        assert_ne!(baseline, terminal_machine_effect_catalog_identity(&changed));
        let mut changed = source.clone();
        let subtract = changed
            .declarations
            .iter_mut()
            .find(|row| row.semantic == TerminalMachineSemanticKind::ExactSubtractI64)
            .unwrap();
        subtract.alternatives[0].applicability =
            TerminalMachineAlternativeApplicability::ResultAliasesOperand {
                result: 2,
                operand: 0,
            };
        assert_ne!(baseline, terminal_machine_effect_catalog_identity(&changed));
        let mut changed = source;
        changed.declarations[0].barrier = TerminalMachineBarrier::ControlFlow;
        assert_ne!(baseline, terminal_machine_effect_catalog_identity(&changed));
    }
}
