use register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
};
use target::NativeTarget;

use super::*;
use crate::{
    MachineAlternative, MachineAlternativeKey, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineLatencyKnowledge, MachineMemoryEffect, MachineSizeKnowledge,
    MachineTrapBehavior, SelectedConstraintKeys,
};

const fn instruction(variant: u32) -> RegisterConstraintKey {
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant,
    }
}

fn keys() -> SelectedConstraintKeys {
    SelectedConstraintKeys {
        hosted_read_byte: Some(instruction(33)),
        hosted_write_byte_i32: Some(instruction(24)),
        hosted_exit_process_i32: Some(instruction(32)),
        load64: Some(instruction(20)),
        load8: Some(instruction(34)),
        load16: Some(instruction(35)),
        load32: Some(instruction(27)),
        load8_indexed: Some(instruction(23)),
        store: Some(instruction(25)),
        address_offset: Some(instruction(26)),
        store64: Some(instruction(21)),
        frame_address: Some(instruction(22)),
        call_unit: vec![RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 2,
        }],
        call_unit_mixed: Vec::new(),
        call_i64: vec![RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 3,
        }],
        materialize_i64: instruction(0),
        copy_i64: instruction(1),
        float32_to_bits: Some(instruction(28)),
        float64_to_bits: Some(instruction(29)),
        bits_to_float32: Some(instruction(30)),
        bits_to_float64: Some(instruction(31)),
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
        memory: if semantic == MachineSemanticKind::HostedWriteByteI32 {
            MachineMemoryEffect::HostedWriteByteV1
        } else {
            MachineMemoryEffect::NoneV1
        },
        trap: if semantic == MachineSemanticKind::HostedExitProcessI32 {
            MachineTrapBehavior::HostedExitReturnedV1
        } else if semantic == MachineSemanticKind::HostedWriteByteI32 {
            MachineTrapBehavior::HostedWriteFailureV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if matches!(
            semantic,
            MachineSemanticKind::HostedWriteByteI32 | MachineSemanticKind::HostedExitProcessI32
        ) {
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
            encoded: if semantic == MachineSemanticKind::HostedExitProcessI32 {
                let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![]);
                encoded.trap = MachineEncodedTrapBehavior::HostedExitReturnedV1;
                encoded.control = MachineEncodedControlEffect::HostedExitOrTrapV1;
                encoded
            } else if semantic == MachineSemanticKind::HostedWriteByteI32 {
                let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![]);
                encoded.memory = MachineEncodedMemoryEffect::HostedWriteByteV1 {
                    stack_pointer: register_model::RegisterViewId(7),
                };
                encoded.trap = MachineEncodedTrapBehavior::HostedWriteFailureV1;
                encoded.control = MachineEncodedControlEffect::HostedWriteReturnOrTrapV1;
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

#[test]
fn identity_distinguishes_narrow_load_keys_and_semantics() {
    let source = catalog();
    let baseline = machine_effect_catalog_identity(&source);
    for mutation in 0..3 {
        let mut changed = source.clone();
        match mutation {
            0 => changed.selected_keys.load8 = None,
            1 => changed.selected_keys.load16 = None,
            _ => {
                let load = changed
                    .declarations
                    .iter_mut()
                    .find(|row| row.semantic == MachineSemanticKind::Load8)
                    .unwrap();
                load.semantic = MachineSemanticKind::Load16;
            }
        }
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
    }
}
