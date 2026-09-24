use super::{
    MachineAlternativeApplicability, MachineAlternativeFamily, MachineBarrier,
    MachineEffectCatalog, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineSemanticKind, alternative_family_tag, encode_machine_alternative_identity,
    encode_machine_alternative_key_identity, encode_machine_encoded_effects_identity,
    machine_effect_catalog_identity, semantic_kind_tag,
};
use crate::SaturatingCarrier;
use crate::{
    MachineAlternative, MachineAlternativeKey, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineLatencyKnowledge, MachineMemoryEffect, MachineSizeKnowledge,
    MachineTrapBehavior, SelectedConstraintKeys,
};
use register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterUnitId, RegisterViewId,
};
use target::NativeTarget;

const fn instruction(variant: u32) -> RegisterConstraintKey {
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant,
    }
}

#[test]
fn crash_identity_tags_do_not_alias_other_instruction_families() {
    for semantic in MachineSemanticKind::ALL {
        if semantic == MachineSemanticKind::Crash {
            continue;
        }
        assert_ne!(
            semantic_kind_tag(MachineSemanticKind::Crash),
            semantic_kind_tag(semantic),
            "{semantic:?}",
        );
        assert_ne!(
            alternative_family_tag(MachineAlternativeFamily::Crash),
            alternative_family_tag(semantic.into()),
            "{semantic:?}",
        );
    }
}

fn keys() -> SelectedConstraintKeys {
    SelectedConstraintKeys {
        crash: instruction(780),
        copy_bytes: Some(instruction(40)),
        save_floating_control: Some(instruction(44)),
        restore_floating_control: Some(instruction(45)),
        call_aggregate: vec![RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 1000,
        }],
        return_aggregate: vec![RegisterConstraintKey {
            family: RegisterConstraintFamily::Return,
            variant: 10,
        }],
        hosted_read_byte: Some(instruction(33)),
        hosted_write_byte_i32: Some(instruction(24)),
        hosted_exit_process_i32: Some(instruction(32)),
        load64: Some(instruction(20)),
        load_packed: Some(instruction(36)),
        store_packed: Some(instruction(37)),
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
        call_scalar: vec![RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 3,
        }],
        call_normalized_foreign: vec![RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 3000,
        }],
        materialize_i64: instruction(0),
        materialize_boolean: instruction(0),
        copy_i64: instruction(1),
        float32_to_bits: Some(instruction(28)),
        float64_to_bits: Some(instruction(29)),
        bits_to_float32: Some(instruction(30)),
        bits_to_float64: Some(instruction(31)),
        add_i64: instruction(2),
        subtract_i64: instruction(4),
        multiply_i64: instruction(44),
        saturating_subtract_unsigned: instruction(4),
        saturating_add_u64: instruction(4),
        divide_u64: instruction(4),
        remainder_u64: instruction(45),
        remainder_i64: instruction(38),
        divide_i64: instruction(46),
        shift_i64: instruction(47),
        saturating_add_clamped: instruction(40),
        saturating_subtract_clamped: instruction(41),
        saturating_divide_signed: instruction(42),
        saturating_multiply_clamped: instruction(48),
        saturating_multiply_u64: instruction(49),
        add_i64_immediate: instruction(3),
        subtract_i64_immediate: instruction(8),
        compare_i64_zero: instruction(5),
        compare_i64: instruction(15),
        compare_i64_immediate: instruction(17),
        conditional_branch: instruction(6),
        jump: instruction(16),
        return_float: Vec::new(),
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
            if semantic == MachineSemanticKind::CallAggregate {
                return keys.call_aggregate.first().copied();
            }
            if semantic == MachineSemanticKind::ReturnAggregate {
                return keys.return_aggregate.first().copied();
            }
            (matches!(
                semantic,
                MachineSemanticKind::CallScalar | MachineSemanticKind::CallUnit
            ))
            .then_some(keys.call_scalar[0])
            .or_else(|| {
                (semantic == MachineSemanticKind::NormalizedForeignCall)
                    .then(|| keys.call_normalized_foreign[0])
            })
        })
        .expect("test catalog declares every semantic constraint");
    MachineEffectDeclaration {
        semantic,
        constraint,
        memory: if semantic == MachineSemanticKind::CopyBytes {
            MachineMemoryEffect::CopyBytesV1
        } else if semantic == MachineSemanticKind::HostedWriteByteI32 {
            MachineMemoryEffect::HostedWriteByteV1
        } else {
            MachineMemoryEffect::NoneV1
        },
        trap: if semantic == MachineSemanticKind::CopyBytes {
            MachineTrapBehavior::MayArchitecturalFaultV1
        } else if semantic == MachineSemanticKind::HostedExitProcessI32 {
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
                | MachineSemanticKind::ReturnScalar
                | MachineSemanticKind::ReturnUnit
                | MachineSemanticKind::ReturnAggregate
        ) {
            MachineBarrier::ControlFlow
        } else if matches!(
            semantic,
            MachineSemanticKind::CallScalar
                | MachineSemanticKind::CallUnit
                | MachineSemanticKind::CallAggregate
                | MachineSemanticKind::NormalizedForeignCall
        ) {
            MachineBarrier::Call
        } else {
            MachineBarrier::None
        },
        call: if semantic == MachineSemanticKind::NormalizedForeignCall {
            MachineCallEffect::DirectExternalNormalReturnV1 {
                pre_call_stack_alignment: 16,
            }
        } else if matches!(
            semantic,
            MachineSemanticKind::CallScalar
                | MachineSemanticKind::CallUnit
                | MachineSemanticKind::CallAggregate
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
            size: MachineSizeKnowledge::ExactBytes(if semantic == MachineSemanticKind::CopyBytes {
                36
            } else {
                4
            }),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: if semantic == MachineSemanticKind::CopyBytes {
                let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0, 1, 2], vec![3, 4]);
                encoded.memory = MachineEncodedMemoryEffect::CopyBytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                    count_operand: 2,
                };
                encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
                encoded.implicit_unit_clobbers = vec![register_model::RegisterUnitId(0)];
                encoded
            } else if semantic == MachineSemanticKind::HostedExitProcessI32 {
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
fn byte_copy_catalog_identity_binds_key_and_both_dynamic_pointers() {
    let source = catalog();
    let baseline = machine_effect_catalog_identity(&source);
    for mutation in 0..3 {
        let mut changed = source.clone();
        if mutation == 0 {
            changed.selected_keys.copy_bytes = None;
        } else {
            let declaration = changed
                .declarations
                .iter_mut()
                .find(|declaration| declaration.semantic == MachineSemanticKind::CopyBytes)
                .unwrap();
            declaration.alternatives[0].encoded.memory = MachineEncodedMemoryEffect::CopyBytesV1 {
                source_pointer_operand: if mutation == 1 { 1 } else { 0 },
                destination_pointer_operand: if mutation == 2 { 0 } else { 1 },
                count_operand: 2,
            };
        }
        assert_ne!(baseline, machine_effect_catalog_identity(&changed));
    }
}

#[test]
fn wrapping_add_catalog_reuses_the_add_constraint_without_exact_semantics() {
    assert_eq!(semantic_kind_tag(MachineSemanticKind::WrappingAddI64), 58);
    assert_eq!(
        alternative_family_tag(MachineAlternativeFamily::WrappingAddI64),
        58
    );
    assert_eq!(
        keys().for_semantic(MachineSemanticKind::WrappingAddI64),
        Some(keys().add_i64)
    );
    let source = catalog();
    let baseline = machine_effect_catalog_identity(&source);
    let mut changed = source;
    let declaration = changed
        .declarations
        .iter_mut()
        .find(|row| row.semantic == MachineSemanticKind::WrappingAddI64)
        .unwrap();
    declaration.semantic = MachineSemanticKind::ExactAddI64;
    assert_ne!(baseline, machine_effect_catalog_identity(&changed));
}

#[test]
fn remainder_catalog_identity_binds_its_key_and_distinct_semantic_family() {
    assert_eq!(semantic_kind_tag(MachineSemanticKind::ExactDivideU64), 56);
    assert_eq!(
        semantic_kind_tag(MachineSemanticKind::WrappingRemainderI64),
        57
    );
    assert_eq!(
        alternative_family_tag(MachineAlternativeFamily::ExactDivideU64),
        56
    );
    assert_eq!(
        alternative_family_tag(MachineAlternativeFamily::WrappingRemainderI64),
        57
    );
    let source = catalog();
    let baseline = machine_effect_catalog_identity(&source);
    let mut changed = source.clone();
    changed.selected_keys.remainder_i64 = instruction(39);
    assert_ne!(baseline, machine_effect_catalog_identity(&changed));
    let mut changed = source;
    let declaration = changed
        .declarations
        .iter_mut()
        .find(|declaration| declaration.semantic == MachineSemanticKind::WrappingRemainderI64)
        .unwrap();
    declaration.semantic = MachineSemanticKind::ExactDivideU64;
    assert_ne!(baseline, machine_effect_catalog_identity(&changed));
}

#[test]
fn widened_arithmetic_semantics_bind_distinct_tags_and_keys() {
    for (semantic, tag) in [
        (MachineSemanticKind::ExactRemainderU64, 89),
        (MachineSemanticKind::WrappingSubtractI64, 90),
        (MachineSemanticKind::WrappingMultiplyI64, 91),
        (MachineSemanticKind::WrappingDivideI64, 92),
        (MachineSemanticKind::BitwiseOrI64, 93),
        (MachineSemanticKind::BitwiseNotI64, 94),
        (MachineSemanticKind::WrappingShiftLeftI64, 105),
        (MachineSemanticKind::WrappingShiftRightI64, 106),
        (MachineSemanticKind::WrappingShiftRightU64, 107),
        (MachineSemanticKind::ExactShiftLeftI64, 108),
        (MachineSemanticKind::ExactShiftRightI64, 109),
        (MachineSemanticKind::ExactShiftRightU64, 110),
        (MachineSemanticKind::ExactDivideI64, 112),
        (MachineSemanticKind::ExactRemainderI64, 113),
    ] {
        assert_eq!(semantic_kind_tag(semantic), tag);
        assert_eq!(
            alternative_family_tag(MachineAlternativeFamily::from(semantic)),
            tag
        );
        assert!(MachineSemanticKind::ALL.contains(&semantic));
    }
    let keys = keys();
    assert_eq!(
        keys.for_semantic(MachineSemanticKind::ExactRemainderU64),
        Some(keys.remainder_u64)
    );
    assert_eq!(
        keys.for_semantic(MachineSemanticKind::WrappingDivideI64),
        Some(keys.divide_i64)
    );
    assert_eq!(
        keys.for_semantic(MachineSemanticKind::WrappingSubtractI64),
        Some(keys.subtract_i64)
    );
    assert_eq!(
        keys.for_semantic(MachineSemanticKind::WrappingMultiplyI64),
        Some(keys.multiply_i64)
    );
    assert_eq!(
        keys.for_semantic(MachineSemanticKind::BitwiseOrI64),
        Some(keys.subtract_i64)
    );
    assert_eq!(
        keys.for_semantic(MachineSemanticKind::BitwiseNotI64),
        Some(keys.copy_i64)
    );
    for semantic in [
        MachineSemanticKind::WrappingShiftLeftI64,
        MachineSemanticKind::WrappingShiftRightI64,
        MachineSemanticKind::WrappingShiftRightU64,
        MachineSemanticKind::ExactShiftLeftI64,
        MachineSemanticKind::ExactShiftRightI64,
        MachineSemanticKind::ExactShiftRightU64,
    ] {
        assert_eq!(keys.for_semantic(semantic), Some(keys.shift_i64));
    }
    let source = catalog();
    let baseline = machine_effect_catalog_identity(&source);
    for mutation in 0..4 {
        let mut changed = source.clone();
        match mutation {
            0 => changed.selected_keys.remainder_u64 = instruction(47),
            1 => changed.selected_keys.divide_i64 = instruction(47),
            2 => {
                changed
                    .declarations
                    .iter_mut()
                    .find(|row| row.semantic == MachineSemanticKind::ExactRemainderU64)
                    .unwrap()
                    .semantic = MachineSemanticKind::ExactDivideU64
            }
            _ => {
                changed
                    .declarations
                    .iter_mut()
                    .find(|row| row.semantic == MachineSemanticKind::WrappingDivideI64)
                    .unwrap()
                    .semantic = MachineSemanticKind::WrappingRemainderI64
            }
        }
        assert_ne!(
            baseline,
            machine_effect_catalog_identity(&changed),
            "mutation {mutation}"
        );
    }
}

#[test]
fn saturating_catalog_identity_binds_keys_and_gives_every_carrier_a_distinct_family() {
    use crate::{SaturatingCarrier, SaturatingOperation, saturating_family_tag};
    // The forms that existed before the family was widened keep their tags;
    // every (operation, carrier) pair has one tag and no two pairs collide.
    assert_eq!(
        saturating_family_tag(SaturatingOperation::Subtract, SaturatingCarrier::U64),
        54
    );
    assert_eq!(
        saturating_family_tag(SaturatingOperation::Add, SaturatingCarrier::U64),
        55
    );
    assert_eq!(
        saturating_family_tag(SaturatingOperation::Add, SaturatingCarrier::I32),
        60
    );
    assert_eq!(
        saturating_family_tag(SaturatingOperation::Subtract, SaturatingCarrier::I32),
        61
    );
    assert_eq!(
        saturating_family_tag(SaturatingOperation::Divide, SaturatingCarrier::I32),
        62
    );
    let mut tags = Vec::new();
    for carrier in SaturatingCarrier::ALL {
        for (semantic, family, operation) in [
            (
                MachineSemanticKind::SaturatingAdd(carrier),
                MachineAlternativeFamily::SaturatingAdd(carrier),
                SaturatingOperation::Add,
            ),
            (
                MachineSemanticKind::SaturatingSubtract(carrier),
                MachineAlternativeFamily::SaturatingSubtract(carrier),
                SaturatingOperation::Subtract,
            ),
            (
                MachineSemanticKind::SaturatingDivide(carrier),
                MachineAlternativeFamily::SaturatingDivide(carrier),
                SaturatingOperation::Divide,
            ),
            (
                MachineSemanticKind::SaturatingRemainder(carrier),
                MachineAlternativeFamily::SaturatingRemainder(carrier),
                SaturatingOperation::Remainder,
            ),
            (
                MachineSemanticKind::SaturatingMultiply(carrier),
                MachineAlternativeFamily::SaturatingMultiply(carrier),
                SaturatingOperation::Multiply,
            ),
        ] {
            let tag = saturating_family_tag(operation, carrier);
            assert_eq!(semantic_kind_tag(semantic), tag);
            assert_eq!(alternative_family_tag(family), tag);
            assert_eq!(MachineAlternativeFamily::from(semantic), family);
            assert!(MachineSemanticKind::ALL.contains(&semantic));
            tags.push(tag);
        }
    }
    let every_tag: std::collections::BTreeSet<u8> = MachineSemanticKind::ALL
        .iter()
        .map(|semantic| semantic_kind_tag(*semantic))
        .collect();
    assert_eq!(every_tag.len(), MachineSemanticKind::ALL.len());
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), 40);
    let source = catalog();
    let baseline = machine_effect_catalog_identity(&source);
    for mutation in 0..9 {
        let mut changed = source.clone();
        match mutation {
            0 => changed.selected_keys.saturating_add_clamped = instruction(43),
            1 => changed.selected_keys.saturating_subtract_clamped = instruction(43),
            2 => changed.selected_keys.saturating_divide_signed = instruction(43),
            7 => changed.selected_keys.saturating_multiply_clamped = instruction(43),
            8 => changed.selected_keys.saturating_multiply_u64 = instruction(43),
            _ => {
                let (from, to) = match mutation {
                    3 => (
                        MachineSemanticKind::SaturatingAdd(SaturatingCarrier::I32),
                        MachineSemanticKind::SaturatingAdd(SaturatingCarrier::U64),
                    ),
                    4 => (
                        MachineSemanticKind::SaturatingSubtract(SaturatingCarrier::I32),
                        MachineSemanticKind::SaturatingAdd(SaturatingCarrier::I32),
                    ),
                    5 => (
                        MachineSemanticKind::SaturatingDivide(SaturatingCarrier::I32),
                        MachineSemanticKind::ExactDivideU64,
                    ),
                    _ => (
                        MachineSemanticKind::SaturatingAdd(SaturatingCarrier::I8),
                        MachineSemanticKind::SaturatingAdd(SaturatingCarrier::I16),
                    ),
                };
                changed
                    .declarations
                    .iter_mut()
                    .find(|declaration| declaration.semantic == from)
                    .unwrap()
                    .semantic = to;
            }
        }
        assert_ne!(
            baseline,
            machine_effect_catalog_identity(&changed),
            "mutation {mutation}"
        );
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
    source
        .selected_keys
        .call_scalar
        .push(RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 4,
        });
    let baseline = machine_effect_catalog_identity(&source);
    let mut reordered = source.clone();
    reordered.selected_keys.call_scalar.swap(0, 1);
    assert_ne!(baseline, machine_effect_catalog_identity(&reordered));
    let mut relabeled = source.clone();
    let structural_key = relabeled.selected_keys.call_unit.remove(0);
    relabeled
        .selected_keys
        .call_scalar
        .insert(0, structural_key);
    assert_eq!(
        source.selected_keys.in_identity_order(),
        relabeled.selected_keys.in_identity_order()
    );
    assert_ne!(baseline, machine_effect_catalog_identity(&relabeled));
}

#[test]
fn identity_distinguishes_foreign_call_and_return_role_boundaries() {
    let source = catalog();
    let mut relabeled = source.clone();
    // `call_normalized_foreign` sits immediately before `return_aggregate` in
    // identity order, so this relabel keeps the flat key sequence identical
    // while moving the key across the call/return roster boundary.
    let call_key = relabeled.selected_keys.call_normalized_foreign.remove(0);
    relabeled.selected_keys.return_aggregate.insert(0, call_key);
    assert_eq!(
        source.selected_keys.in_identity_order(),
        relabeled.selected_keys.in_identity_order()
    );
    assert_ne!(
        machine_effect_catalog_identity(&source),
        machine_effect_catalog_identity(&relabeled)
    );
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

/// Expected tags ported verbatim from the eighty-one-arm copy of this table
/// that `machine_code::layout::identity` carried before the copies were
/// removed, expanded over every family in
/// [`MachineSemanticKind::ALL`]. These bytes are the identity of an
/// alternative in every catalog, program, physical-instruction and
/// machine-code artifact, so this table changing is always a defect.
const PINNED_ALTERNATIVE_FAMILY_TAGS: [(MachineAlternativeFamily, u8); 117] = [
    (MachineAlternativeFamily::Crash, 111),
    (MachineAlternativeFamily::CopyBytes, 59),
    (MachineAlternativeFamily::BitwiseAndI64, 51),
    (MachineAlternativeFamily::BitwiseXorI64, 52),
    (MachineAlternativeFamily::CallAggregate, 35),
    (MachineAlternativeFamily::ReturnAggregate, 36),
    (MachineAlternativeFamily::HostedExitProcessI32, 31),
    (MachineAlternativeFamily::LoadPacked3, 46),
    (MachineAlternativeFamily::LoadPacked5, 47),
    (MachineAlternativeFamily::LoadPacked6, 48),
    (MachineAlternativeFamily::LoadPacked7, 49),
    (MachineAlternativeFamily::StorePacked, 50),
    (MachineAlternativeFamily::Load8, 33),
    (MachineAlternativeFamily::Load16, 34),
    (MachineAlternativeFamily::Load32, 30),
    (MachineAlternativeFamily::Float32ToBits, 26),
    (MachineAlternativeFamily::Float64ToBits, 27),
    (MachineAlternativeFamily::BitsToFloat32, 28),
    (MachineAlternativeFamily::BitsToFloat64, 29),
    (MachineAlternativeFamily::Load8Indexed, 21),
    (MachineAlternativeFamily::CompareI64Zero, 0),
    (MachineAlternativeFamily::MaterializeI64, 1),
    (MachineAlternativeFamily::CopyI64, 2),
    (MachineAlternativeFamily::ExactAddI64, 3),
    (MachineAlternativeFamily::ExactAddI64Immediate, 4),
    (MachineAlternativeFamily::ExactSubtractI64, 5),
    (MachineAlternativeFamily::ExactDivideU64, 56),
    (MachineAlternativeFamily::ExactSubtractI64Immediate, 8),
    (MachineAlternativeFamily::ConditionalBranchNonZero, 6),
    (MachineAlternativeFamily::ReturnScalar, 7),
    (MachineAlternativeFamily::ReturnUnit, 9),
    (MachineAlternativeFamily::CompareI64, 10),
    (MachineAlternativeFamily::CompareI64Immediate, 53),
    (MachineAlternativeFamily::ConditionalBranchU64LessThan, 11),
    (MachineAlternativeFamily::ConditionalBranchI64LessThan, 12),
    (MachineAlternativeFamily::CallScalar, 13),
    (MachineAlternativeFamily::Jump, 14),
    (MachineAlternativeFamily::ZeroExtendU8, 15),
    (MachineAlternativeFamily::ZeroExtendU32, 20),
    (MachineAlternativeFamily::ZeroExtendU16, 37),
    (MachineAlternativeFamily::SignExtendI8, 38),
    (MachineAlternativeFamily::SignExtendI16, 39),
    (MachineAlternativeFamily::SignExtendI32, 40),
    (MachineAlternativeFamily::Load64, 16),
    (MachineAlternativeFamily::Store64, 17),
    (MachineAlternativeFamily::FrameAddress, 18),
    (MachineAlternativeFamily::CallUnit, 19),
    (MachineAlternativeFamily::ByteViewAddress, 22),
    (MachineAlternativeFamily::HostedReadByte, 32),
    (MachineAlternativeFamily::HostedWriteByteI32, 23),
    (MachineAlternativeFamily::Store, 24),
    (MachineAlternativeFamily::AddressOffset, 25),
    (MachineAlternativeFamily::MaterializeBooleanEqual, 41),
    (MachineAlternativeFamily::MaterializeBooleanU64LessThan, 42),
    (MachineAlternativeFamily::MaterializeBooleanI64LessThan, 43),
    (
        MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual,
        44,
    ),
    (
        MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual,
        45,
    ),
    (MachineAlternativeFamily::WrappingRemainderI64, 57),
    (MachineAlternativeFamily::WrappingAddI64, 58),
    (
        MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::I8),
        63,
    ),
    (
        MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::I16),
        64,
    ),
    (
        MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::I32),
        60,
    ),
    (
        MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::I64),
        66,
    ),
    (
        MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::U8),
        67,
    ),
    (
        MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::U16),
        68,
    ),
    (
        MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::U32),
        69,
    ),
    (
        MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::U64),
        55,
    ),
    (
        MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::I8),
        71,
    ),
    (
        MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::I16),
        72,
    ),
    (
        MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::I32),
        61,
    ),
    (
        MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::I64),
        74,
    ),
    (
        MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::U8),
        75,
    ),
    (
        MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::U16),
        76,
    ),
    (
        MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::U32),
        77,
    ),
    (
        MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::U64),
        54,
    ),
    (
        MachineAlternativeFamily::SaturatingDivide(SaturatingCarrier::I8),
        79,
    ),
    (
        MachineAlternativeFamily::SaturatingDivide(SaturatingCarrier::I16),
        80,
    ),
    (
        MachineAlternativeFamily::SaturatingDivide(SaturatingCarrier::I32),
        62,
    ),
    (
        MachineAlternativeFamily::SaturatingDivide(SaturatingCarrier::I64),
        82,
    ),
    (
        MachineAlternativeFamily::SaturatingDivide(SaturatingCarrier::U8),
        83,
    ),
    (
        MachineAlternativeFamily::SaturatingDivide(SaturatingCarrier::U16),
        84,
    ),
    (
        MachineAlternativeFamily::SaturatingDivide(SaturatingCarrier::U32),
        85,
    ),
    (
        MachineAlternativeFamily::SaturatingDivide(SaturatingCarrier::U64),
        86,
    ),
    (
        MachineAlternativeFamily::SaturatingRemainder(SaturatingCarrier::I8),
        95,
    ),
    (
        MachineAlternativeFamily::SaturatingRemainder(SaturatingCarrier::I16),
        96,
    ),
    (
        MachineAlternativeFamily::SaturatingRemainder(SaturatingCarrier::I32),
        97,
    ),
    (
        MachineAlternativeFamily::SaturatingRemainder(SaturatingCarrier::I64),
        98,
    ),
    (
        MachineAlternativeFamily::SaturatingRemainder(SaturatingCarrier::U8),
        99,
    ),
    (
        MachineAlternativeFamily::SaturatingRemainder(SaturatingCarrier::U16),
        100,
    ),
    (
        MachineAlternativeFamily::SaturatingRemainder(SaturatingCarrier::U32),
        101,
    ),
    (
        MachineAlternativeFamily::SaturatingRemainder(SaturatingCarrier::U64),
        102,
    ),
    (MachineAlternativeFamily::NormalizedForeignCall, 87),
    (MachineAlternativeFamily::ExactMultiplyI64, 88),
    (MachineAlternativeFamily::ExactRemainderU64, 89),
    (MachineAlternativeFamily::WrappingSubtractI64, 90),
    (MachineAlternativeFamily::WrappingMultiplyI64, 91),
    (MachineAlternativeFamily::WrappingDivideI64, 92),
    (MachineAlternativeFamily::BitwiseOrI64, 93),
    (MachineAlternativeFamily::BitwiseNotI64, 94),
    (MachineAlternativeFamily::SaveFloatingControl, 103),
    (MachineAlternativeFamily::RestoreFloatingControl, 104),
    (MachineAlternativeFamily::WrappingShiftLeftI64, 105),
    (MachineAlternativeFamily::WrappingShiftRightI64, 106),
    (MachineAlternativeFamily::WrappingShiftRightU64, 107),
    (MachineAlternativeFamily::ExactShiftLeftI64, 108),
    (MachineAlternativeFamily::ExactShiftRightI64, 109),
    (MachineAlternativeFamily::ExactShiftRightU64, 110),
    (MachineAlternativeFamily::ExactDivideI64, 112),
    (MachineAlternativeFamily::ExactRemainderI64, 113),
    (
        MachineAlternativeFamily::SaturatingMultiply(SaturatingCarrier::I8),
        114,
    ),
    (
        MachineAlternativeFamily::SaturatingMultiply(SaturatingCarrier::I16),
        115,
    ),
    (
        MachineAlternativeFamily::SaturatingMultiply(SaturatingCarrier::I32),
        116,
    ),
    (
        MachineAlternativeFamily::SaturatingMultiply(SaturatingCarrier::I64),
        117,
    ),
    (
        MachineAlternativeFamily::SaturatingMultiply(SaturatingCarrier::U8),
        118,
    ),
    (
        MachineAlternativeFamily::SaturatingMultiply(SaturatingCarrier::U16),
        119,
    ),
    (
        MachineAlternativeFamily::SaturatingMultiply(SaturatingCarrier::U32),
        120,
    ),
    (
        MachineAlternativeFamily::SaturatingMultiply(SaturatingCarrier::U64),
        121,
    ),
];

#[test]
fn alternative_family_tag_is_pinned_for_every_family() {
    for (family, expected) in PINNED_ALTERNATIVE_FAMILY_TAGS {
        assert_eq!(alternative_family_tag(family), expected, "{family:?}");
    }
    for semantic in MachineSemanticKind::ALL {
        let family = MachineAlternativeFamily::from(semantic);
        assert!(
            PINNED_ALTERNATIVE_FAMILY_TAGS
                .iter()
                .any(|(pinned, _)| *pinned == family),
            "{family:?} has no pinned identity tag",
        );
    }
}

#[test]
fn machine_alternative_key_identity_bytes_are_pinned() {
    for (family, tag) in PINNED_ALTERNATIVE_FAMILY_TAGS {
        let mut bytes = Vec::new();
        encode_machine_alternative_key_identity(
            &mut bytes,
            MachineAlternativeKey {
                family,
                variant: 0x0403_0201,
            },
        );
        assert_eq!(bytes, vec![tag, 0x01, 0x02, 0x03, 0x04], "{family:?}");
    }
}

fn pinned_effects(
    memory: MachineEncodedMemoryEffect,
    stack: MachineEncodedStackEffect,
    trap: MachineEncodedTrapBehavior,
    control: MachineEncodedControlEffect,
) -> MachineEncodedEffects {
    MachineEncodedEffects {
        external_operand_reads: Vec::new(),
        external_operand_writes: Vec::new(),
        implicit_unit_uses: Vec::new(),
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: Vec::new(),
        memory,
        stack,
        trap,
        control,
    }
}

/// Five empty lists, each written as a little-endian `u64` length.
const EMPTY_LIST_PREFIX: [u8; 40] = [0; 40];

fn encoded_identity(effects: &MachineEncodedEffects) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_machine_encoded_effects_identity(&mut bytes, effects);
    bytes
}

#[test]
fn encoded_effect_lists_are_length_prefixed_in_pinned_order() {
    let mut effects = pinned_effects(
        MachineEncodedMemoryEffect::NoneV1,
        MachineEncodedStackEffect::UnchangedV1,
        MachineEncodedTrapBehavior::NeverV1,
        MachineEncodedControlEffect::FallThroughV1,
    );
    effects.external_operand_reads = vec![0x0201];
    effects.external_operand_writes = vec![0x0403, 0x0605];
    effects.implicit_unit_uses = vec![RegisterUnitId(0x0807)];
    effects.implicit_unit_defs = vec![RegisterUnitId(0x0a09)];
    effects.implicit_unit_clobbers = vec![RegisterUnitId(0x0c0b)];
    assert_eq!(
        encoded_identity(&effects),
        vec![
            1, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x02, //
            2, 0, 0, 0, 0, 0, 0, 0, 0x03, 0x04, 0x05, 0x06, //
            1, 0, 0, 0, 0, 0, 0, 0, 0x07, 0x08, //
            1, 0, 0, 0, 0, 0, 0, 0, 0x09, 0x0a, //
            1, 0, 0, 0, 0, 0, 0, 0, 0x0b, 0x0c, //
            0, 0, 0, 0,
        ],
    );
}

#[test]
fn encoded_memory_effect_identity_bytes_are_pinned() {
    let cases: [(MachineEncodedMemoryEffect, &[u8]); 11] = [
        (MachineEncodedMemoryEffect::NoneV1, &[0]),
        (
            MachineEncodedMemoryEffect::ReadActivationStackV1 {
                stack_pointer: RegisterViewId(0x0201),
                byte_count: 0x0403,
            },
            &[1, 0x01, 0x02, 0x03, 0x04],
        ),
        (
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer: RegisterViewId(0x0201),
                byte_count: 0x0403,
            },
            &[2, 0x01, 0x02, 0x03, 0x04],
        ),
        (
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0x0201,
                byte_count: 0x0403,
            },
            &[3, 0x01, 0x02, 0x03, 0x04],
        ),
        (
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                stack_pointer: RegisterViewId(0x0201),
                byte_count: 0x0403,
            },
            &[4, 0x01, 0x02, 0x03, 0x04],
        ),
        (
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand: 0x0201,
                index_operand: 0x0403,
                byte_count: 0x0605,
            },
            &[5, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        ),
        (
            MachineEncodedMemoryEffect::HostedWriteByteV1 {
                stack_pointer: RegisterViewId(0x0201),
            },
            &[6, 0x01, 0x02],
        ),
        (
            MachineEncodedMemoryEffect::WritePointerV1 {
                pointer_operand: 0x0201,
            },
            &[7, 0x01, 0x02],
        ),
        (
            MachineEncodedMemoryEffect::HostedReadByteV1 {
                stack_pointer: RegisterViewId(0x0201),
            },
            &[8, 0x01, 0x02],
        ),
        (
            MachineEncodedMemoryEffect::CopyBytesV1 {
                source_pointer_operand: 0x0201,
                destination_pointer_operand: 0x0403,
                count_operand: 0x0605,
            },
            &[9, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        ),
        (
            MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                stack_pointer: RegisterViewId(0x0201),
                byte_count: 0x0403,
            },
            &[10, 0x01, 0x02, 0x03, 0x04],
        ),
    ];
    for (memory, expected) in cases {
        let mut wanted = EMPTY_LIST_PREFIX.to_vec();
        wanted.extend_from_slice(expected);
        wanted.extend_from_slice(&[0, 0, 0]);
        assert_eq!(
            encoded_identity(&pinned_effects(
                memory,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            )),
            wanted,
            "{memory:?}",
        );
    }
}

#[test]
fn encoded_stack_effect_identity_bytes_are_pinned() {
    let cases: [(MachineEncodedStackEffect, &[u8]); 3] = [
        (MachineEncodedStackEffect::UnchangedV1, &[0]),
        (
            MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer: RegisterViewId(0x0201),
                byte_count: 0x0403,
            },
            &[1, 0x01, 0x02, 0x03, 0x04],
        ),
        (
            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                stack_pointer: RegisterViewId(0x0201),
                return_address_byte_count: 0x0403,
            },
            &[2, 0x01, 0x02, 0x03, 0x04],
        ),
    ];
    for (stack, expected) in cases {
        let mut wanted = EMPTY_LIST_PREFIX.to_vec();
        wanted.push(0);
        wanted.extend_from_slice(expected);
        wanted.extend_from_slice(&[0, 0]);
        assert_eq!(
            encoded_identity(&pinned_effects(
                MachineEncodedMemoryEffect::NoneV1,
                stack,
                MachineEncodedTrapBehavior::NeverV1,
                MachineEncodedControlEffect::FallThroughV1,
            )),
            wanted,
            "{stack:?}",
        );
    }
}

#[test]
fn encoded_trap_behavior_identity_bytes_are_pinned() {
    let cases: [(MachineEncodedTrapBehavior, u8); 6] = [
        (MachineEncodedTrapBehavior::NeverV1, 0),
        (MachineEncodedTrapBehavior::MayArchitecturalFaultV1, 1),
        (MachineEncodedTrapBehavior::HostedWriteFailureV1, 2),
        (MachineEncodedTrapBehavior::HostedExitReturnedV1, 3),
        (MachineEncodedTrapBehavior::HostedReadFailureV1, 4),
        (MachineEncodedTrapBehavior::ExplicitCrashV1, 5),
    ];
    for (trap, expected) in cases {
        let mut wanted = EMPTY_LIST_PREFIX.to_vec();
        wanted.extend_from_slice(&[0, 0, expected, 0]);
        assert_eq!(
            encoded_identity(&pinned_effects(
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                trap,
                MachineEncodedControlEffect::FallThroughV1,
            )),
            wanted,
            "{trap:?}",
        );
    }
}

#[test]
fn encoded_control_effect_identity_bytes_are_pinned() {
    let cases: [(MachineEncodedControlEffect, &[u8]); 10] = [
        (MachineEncodedControlEffect::FallThroughV1, &[0]),
        (
            MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            &[1],
        ),
        (
            MachineEncodedControlEffect::ReturnFromActivationStackV1,
            &[2],
        ),
        (
            MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                target: RegisterViewId(0x0201),
            },
            &[3, 0x01, 0x02],
        ),
        (MachineEncodedControlEffect::DirectRelativeCallV1, &[4]),
        (
            MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
            &[5],
        ),
        (MachineEncodedControlEffect::HostedWriteReturnOrTrapV1, &[6]),
        (MachineEncodedControlEffect::HostedExitOrTrapV1, &[7]),
        (MachineEncodedControlEffect::HostedReadReturnOrTrapV1, &[8]),
        (MachineEncodedControlEffect::CrashV1, &[9]),
    ];
    for (control, expected) in cases {
        let mut wanted = EMPTY_LIST_PREFIX.to_vec();
        wanted.extend_from_slice(&[0, 0, 0]);
        wanted.extend_from_slice(expected);
        assert_eq!(
            encoded_identity(&pinned_effects(
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedStackEffect::UnchangedV1,
                MachineEncodedTrapBehavior::NeverV1,
                control,
            )),
            wanted,
            "{control:?}",
        );
    }
}

/// The pinned tail of an alternative whose encoded effects are all empty or
/// `None`: five empty lists then the four neutral effect tags.
fn pinned_neutral_effects_tail() -> Vec<u8> {
    let mut tail = EMPTY_LIST_PREFIX.to_vec();
    tail.extend_from_slice(&[0, 0, 0, 0]);
    tail
}

fn pinned_alternative(
    applicability: MachineAlternativeApplicability,
    size: MachineSizeKnowledge,
) -> MachineAlternative {
    MachineAlternative {
        key: MachineAlternativeKey {
            family: MachineAlternativeFamily::CompareI64Zero,
            variant: 0x0403_0201,
        },
        applicability,
        size,
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: pinned_effects(
            MachineEncodedMemoryEffect::NoneV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::NeverV1,
            MachineEncodedControlEffect::FallThroughV1,
        ),
    }
}

#[test]
fn machine_alternative_identity_bytes_are_pinned_for_every_variant() {
    // Expected bytes ported from the copy of this body that
    // `physical_instructions::identity` carried before de-duplication: family
    // tag, little-endian variant, applicability, size, latency, encoded
    // effects.
    let applicabilities: [(MachineAlternativeApplicability, &[u8]); 6] = [
        (MachineAlternativeApplicability::Always, &[0]),
        (
            MachineAlternativeApplicability::ResultAliasesOperand {
                result: 0x0201,
                operand: 0x0403,
            },
            &[1, 0x01, 0x02, 0x03, 0x04],
        ),
        (
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 0x0201,
                aliased_operand: 0x0403,
                distinct_operand: 0x0605,
            },
            &[2, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        ),
        (
            MachineAlternativeApplicability::ResultAliasesOperands {
                result: 0x0201,
                left: 0x0403,
                right: 0x0605,
            },
            &[3, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        ),
        (
            MachineAlternativeApplicability::ResultDistinctFromOperands {
                result: 0x0201,
                left: 0x0403,
                right: 0x0605,
            },
            &[4, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        ),
        (
            MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
                left: 0x0201,
                right: 0x0403,
                excluded_view: RegisterViewId(0x0605),
            },
            &[5, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        ),
    ];
    let sizes: [(MachineSizeKnowledge, &[u8]); 3] = [
        (MachineSizeKnowledge::ExactBytes(0x0201), &[0, 0x01, 0x02]),
        (
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 0x0201,
                maximum_bytes: None,
            },
            &[1, 0x01, 0x02, 0],
        ),
        (
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 0x0201,
                maximum_bytes: Some(0x0403),
            },
            &[1, 0x01, 0x02, 1, 0x03, 0x04],
        ),
    ];
    for (applicability, applicability_bytes) in applicabilities {
        for (size, size_bytes) in sizes {
            let mut expected = vec![
                alternative_family_tag(MachineAlternativeFamily::CompareI64Zero),
                0x01,
                0x02,
                0x03,
                0x04,
            ];
            expected.extend_from_slice(applicability_bytes);
            expected.extend_from_slice(size_bytes);
            // Latency: the only knowledge variant.
            expected.push(0);
            expected.extend_from_slice(&pinned_neutral_effects_tail());
            let mut bytes = Vec::new();
            encode_machine_alternative_identity(
                &mut bytes,
                &pinned_alternative(applicability, size),
            );
            assert_eq!(bytes, expected, "{applicability:?} {size:?}");
        }
    }
}
