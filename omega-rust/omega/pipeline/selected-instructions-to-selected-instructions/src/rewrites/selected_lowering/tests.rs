use optimization_core::{
    AcceptedObligationFactIdentity, Optimization, OptimizationExecutionPhase,
    OptimizationSelections,
};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{MachineSemanticKind, SelectedInstructionKind};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ObligationId, ScalarType};
use target::NativeTarget;

use super::{
    LiteralFoldPolicy, ORDERED_SELECTED_LOWERING_RULES, PairMachineEffects, PairOperandShape,
    PairResultDisposition, PairUnitEffects, SELECTED_LOWERING_RULE_CATALOG,
    SelectedInstructionPairRule, enabled_pair_rules, resolve_selected_lowering_rules,
};
use crate::{RegisterAllocationRuleTargetApplicability, validated_machine_effect_catalog};

#[test]
fn catalog_exactly_matches_the_selected_lowering_vocabulary() {
    let declared = Optimization::ALL
        .into_iter()
        .filter(|optimization| {
            optimization.execution_phase() == OptimizationExecutionPhase::SelectedLowering
        })
        .collect::<Vec<_>>();
    assert_eq!(declared, ORDERED_SELECTED_LOWERING_RULES);
    assert_eq!(
        SELECTED_LOWERING_RULE_CATALOG.map(|entry| entry.optimization()),
        ORDERED_SELECTED_LOWERING_RULES,
    );
    assert!(SELECTED_LOWERING_RULE_CATALOG.iter().all(|entry| {
        entry.payload().target() == RegisterAllocationRuleTargetApplicability::TargetIndependent
    }));
    for entry in SELECTED_LOWERING_RULE_CATALOG {
        let selections = OptimizationSelections::new([entry.optimization()]).unwrap();
        let phase = selections.project_phase(OptimizationExecutionPhase::SelectedLowering);
        let (selected, policy) = resolve_selected_lowering_rules(&phase).unwrap();
        assert_eq!(selected, selections);
        assert_eq!(policy, entry.payload().policy());
    }
    let composition = OptimizationSelections::new(ORDERED_SELECTED_LOWERING_RULES).unwrap();
    let phase = composition.project_phase(OptimizationExecutionPhase::SelectedLowering);
    let (selected, policy) = resolve_selected_lowering_rules(&phase).unwrap();
    assert_eq!(selected.as_slice(), ORDERED_SELECTED_LOWERING_RULES);
    assert_eq!(
        policy,
        SELECTED_LOWERING_RULE_CATALOG
            .into_iter()
            .fold(LiteralFoldPolicy::empty(), |resolved, entry| resolved
                .union(entry.payload().policy()))
    );
    assert!(policy.enables_exact_add());
    assert!(policy.enables_exact_subtract());
    assert!(policy.enables_compare());
    assert!(policy.enables_extension());
    assert!(policy.enables_load8_indexed());
    assert!(policy.enables_copy());
    assert!(policy.enables_byte_view_address());
}

#[test]
fn catalog_rows_declare_symbolic_instruction_pairs() {
    let [
        add,
        subtract,
        compare,
        _extension,
        indexed,
        copy,
        address_offset,
    ] = SELECTED_LOWERING_RULE_CATALOG;
    for entry in [subtract, compare] {
        let &[pair] = entry.payload().pairs() else {
            panic!("the subtract and compare families each declare one pair rule")
        };
        assert_eq!(pair.producer(), MachineSemanticKind::MaterializeI64);
        assert_eq!(pair.immediate_limit(), 4095);
        assert!(pair.admits_immediate(4095));
        assert!(!pair.admits_immediate(4096));
        assert_eq!(pair.operand_shape(), PairOperandShape::BinaryRightLiteral);
        assert_eq!(pair.victim_operand(), 1);
    }
    // Exact addition commutes, so its family admits the folded literal at
    // either `Use` position of the same consumer kind and rewrites through
    // the same immediate row.
    let &[add_rule, add_left_rule] = add.payload().pairs() else {
        panic!("exact-add declares one pair per operand grammar")
    };
    let &[subtract_rule] = subtract.payload().pairs() else {
        panic!("exact-subtract declares one pair rule")
    };
    for pair in [add_rule, add_left_rule] {
        assert_eq!(pair.producer(), MachineSemanticKind::MaterializeI64);
        assert_eq!(pair.immediate_limit(), 4095);
        assert!(pair.admits_immediate(4095));
        assert!(!pair.admits_immediate(4096));
        assert_eq!(pair.consumer(), MachineSemanticKind::ExactAddI64);
        assert_eq!(pair.rewritten(), MachineSemanticKind::ExactAddI64Immediate);
        assert_eq!(pair.result(), PairResultDisposition::ScalarRegister);
        assert_eq!(pair.unit_effects(), PairUnitEffects::Isolated);
        assert_eq!(pair.machine_effects(), PairMachineEffects::Isolated);
    }
    assert_eq!(
        add_rule,
        SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12
    );
    assert_eq!(
        add_rule.operand_shape(),
        PairOperandShape::BinaryRightLiteral
    );
    assert_eq!(add_rule.victim_operand(), 1);
    assert_eq!(
        add_left_rule,
        SelectedInstructionPairRule::EXACT_ADD_LEFT_IMMEDIATE_U12
    );
    assert_eq!(
        add_left_rule.operand_shape(),
        PairOperandShape::BinaryLeftLiteral
    );
    assert_eq!(add_left_rule.victim_operand(), 0);
    assert_eq!(
        subtract_rule,
        SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12
    );
    assert_eq!(
        subtract_rule.consumer(),
        MachineSemanticKind::ExactSubtractI64
    );
    assert_eq!(
        subtract_rule.rewritten(),
        MachineSemanticKind::ExactSubtractI64Immediate
    );
    assert_eq!(
        subtract_rule.result(),
        PairResultDisposition::ScalarRegister
    );
    assert_ne!(add_rule.consumer(), subtract_rule.consumer());

    let &[compare_rule] = compare.payload().pairs() else {
        panic!("compare declares one pair rule")
    };
    assert_eq!(
        compare_rule,
        SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12
    );
    assert_eq!(compare_rule.consumer(), MachineSemanticKind::CompareI64);
    assert_eq!(
        compare_rule.rewritten(),
        MachineSemanticKind::CompareI64Immediate
    );
    // The compare-immediate form delivers its result through the implicit
    // physical-unit condition state, not a scalar `Def` operand.
    assert_eq!(compare_rule.result(), PairResultDisposition::ImplicitUnits);
    assert_eq!(
        compare_rule.rewrite_consumer(SelectedInstructionKind::CompareI64, 12, None),
        Some(SelectedInstructionKind::CompareI64Immediate {
            immediate: IntegerValue::Unsigned(12),
        })
    );

    // The indexed byte-load family declares the first non-isolated
    // machine-effect relationship: the folded operand is the index register
    // and the rewritten form is the direct-offset `Load8`.
    let &[indexed_rule] = indexed.payload().pairs() else {
        panic!("the indexed byte-load family declares one pair rule")
    };
    assert_eq!(indexed_rule, SelectedInstructionPairRule::LOAD8_INDEXED_U12);
    assert_eq!(indexed_rule.producer(), MachineSemanticKind::MaterializeI64);
    assert_eq!(indexed_rule.consumer(), MachineSemanticKind::Load8Indexed);
    assert_eq!(indexed_rule.rewritten(), MachineSemanticKind::Load8);
    assert_eq!(indexed_rule.immediate_limit(), 4095);
    assert!(indexed_rule.admits_immediate(4095));
    assert!(!indexed_rule.admits_immediate(4096));
    assert_eq!(
        indexed_rule.operand_shape(),
        PairOperandShape::BinaryRightLiteral
    );
    assert_eq!(indexed_rule.victim_operand(), 1);
    assert_eq!(indexed_rule.result(), PairResultDisposition::ScalarRegister);
    assert_eq!(indexed_rule.unit_effects(), PairUnitEffects::Isolated);
    assert_eq!(
        indexed_rule.machine_effects(),
        PairMachineEffects::IndexedPointerReadFold { index_operand: 1 }
    );
    assert_eq!(
        indexed_rule.rewrite_consumer(SelectedInstructionKind::Load8Indexed, 12, None),
        Some(SelectedInstructionKind::Load8 { byte_offset: 12 })
    );
    // Payloads that cannot encode a 32-bit byte offset cannot rewrite, even
    // though the 12-bit fold bound already rejects them upstream.
    assert_eq!(
        indexed_rule.rewrite_consumer(SelectedInstructionKind::Load8Indexed, u64::MAX, None),
        None
    );
    assert_eq!(
        indexed_rule.rewrite_consumer(SelectedInstructionKind::Load8 { byte_offset: 1 }, 12, None),
        None
    );

    // The copy-materialization family declares one unary materialization
    // rule: `MaterializeI64` feeding `CopyI64` folds to a direct
    // `MaterializeI64` of the literal at the copy's destination.
    let &[copy_rule] = copy.payload().pairs() else {
        panic!("the copy-materialization family declares one pair rule")
    };
    assert_eq!(copy_rule, SelectedInstructionPairRule::COPY_LITERAL_FOLD);
    assert_eq!(copy_rule.producer(), MachineSemanticKind::MaterializeI64);
    assert_eq!(copy_rule.consumer(), MachineSemanticKind::CopyI64);
    assert_eq!(copy_rule.rewritten(), MachineSemanticKind::MaterializeI64);
    assert_eq!(copy_rule.operand_shape(), PairOperandShape::UnaryLiteral);
    assert_eq!(copy_rule.victim_operand(), 0);
    assert_eq!(copy_rule.result(), PairResultDisposition::ScalarRegister);
    assert_eq!(copy_rule.unit_effects(), PairUnitEffects::Isolated);
    assert_eq!(copy_rule.machine_effects(), PairMachineEffects::Isolated);
    assert_eq!(copy_rule.immediate_limit(), u64::MAX);

    // The byte-view address family folds the projection's operand-1 offset
    // literal into the constant-offset `AddressOffset` form: the operand-0
    // `Use` base survives and the operand-2 `Def` is the result. The shared
    // 12-bit bound is the aarch64 `add` immediate's encoding limit, the
    // narrowest any target's address-offset row admits.
    let &[address_offset_rule] = address_offset.payload().pairs() else {
        panic!("the byte-view address family declares one pair rule")
    };
    assert_eq!(
        address_offset_rule,
        SelectedInstructionPairRule::BYTE_VIEW_ADDRESS_OFFSET_U12
    );
    assert_eq!(
        address_offset_rule.producer(),
        MachineSemanticKind::MaterializeI64
    );
    assert_eq!(
        address_offset_rule.consumer(),
        MachineSemanticKind::ByteViewAddress
    );
    assert_eq!(
        address_offset_rule.rewritten(),
        MachineSemanticKind::AddressOffset
    );
    assert_eq!(address_offset_rule.immediate_limit(), 4095);
    assert!(address_offset_rule.admits_immediate(4095));
    assert!(!address_offset_rule.admits_immediate(4096));
    assert_eq!(
        address_offset_rule.operand_shape(),
        PairOperandShape::BinaryRightLiteral
    );
    assert_eq!(address_offset_rule.victim_operand(), 1);
    assert_eq!(
        address_offset_rule.result(),
        PairResultDisposition::ScalarRegister
    );
    assert_eq!(
        address_offset_rule.unit_effects(),
        PairUnitEffects::Isolated
    );
    assert_eq!(
        address_offset_rule.machine_effects(),
        PairMachineEffects::Isolated
    );
    assert_eq!(
        address_offset_rule.rewrite_consumer(SelectedInstructionKind::ByteViewAddress, 12, None),
        Some(SelectedInstructionKind::AddressOffset { byte_offset: 12 })
    );
    assert_eq!(
        address_offset_rule.rewrite_consumer(
            SelectedInstructionKind::ByteViewAddress,
            u64::MAX,
            None
        ),
        None
    );
    assert_eq!(
        address_offset_rule.rewrite_consumer(
            SelectedInstructionKind::AddressOffset { byte_offset: 1 },
            12,
            None
        ),
        None
    );

    // Every landed rule's rewrite is unit-effect isolated: no implicit unit
    // uses or clobbers and no operand unit bindings beyond the declared
    // result channel.
    for entry in [add, subtract, compare, indexed, copy, address_offset] {
        for pair in entry.payload().pairs() {
            assert_eq!(pair.unit_effects(), PairUnitEffects::Isolated);
        }
    }

    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::EXACT_ADD_V1).collect::<Vec<_>>(),
        vec![
            SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12,
            SelectedInstructionPairRule::EXACT_ADD_LEFT_IMMEDIATE_U12,
        ]
    );
    let union = LiteralFoldPolicy::EXACT_ADD_V1.union(LiteralFoldPolicy::EXACT_SUBTRACT_V1);
    assert_eq!(
        enabled_pair_rules(union).collect::<Vec<_>>(),
        vec![
            SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12,
            SelectedInstructionPairRule::EXACT_ADD_LEFT_IMMEDIATE_U12,
            SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12,
        ]
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::COMPARE_V1).collect::<Vec<_>>(),
        vec![SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12]
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::EXTENSION_V1).collect::<Vec<_>>(),
        SelectedInstructionPairRule::EXTENSION_LITERAL_FOLDS.to_vec()
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::LOAD8_INDEXED_V1).collect::<Vec<_>>(),
        vec![SelectedInstructionPairRule::LOAD8_INDEXED_U12]
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::COPY_V1).collect::<Vec<_>>(),
        vec![SelectedInstructionPairRule::COPY_LITERAL_FOLD]
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1).collect::<Vec<_>>(),
        vec![SelectedInstructionPairRule::BYTE_VIEW_ADDRESS_OFFSET_U12]
    );
    assert_eq!(enabled_pair_rules(LiteralFoldPolicy::empty()).count(), 0);

    let obligation = ObligationId::new(7).unwrap();
    let accepted_fact = AcceptedObligationFactIdentity::from_bytes([9; 32]);
    let add_kind = SelectedInstructionKind::ExactAddI64 {
        obligation,
        accepted_fact,
    };
    let subtract_kind = SelectedInstructionKind::ExactSubtractI64 {
        obligation,
        accepted_fact,
    };
    assert_eq!(add_rule.rewrite_consumer(subtract_kind, 12, None), None);
    assert_eq!(
        add_rule.rewrite_consumer(add_kind, 12, None),
        Some(SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(12),
            obligation,
            accepted_fact,
        })
    );
    // The left grammar rewrites through the same immediate form: the folded
    // payload and proof custody are operand-position independent.
    assert_eq!(
        add_left_rule.rewrite_consumer(add_kind, 12, None),
        add_rule.rewrite_consumer(add_kind, 12, None)
    );

    let materialize = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(3),
    };
    assert!(add_rule.matches_consumer(add_kind));
    assert!(!add_rule.matches_consumer(subtract_kind));
    assert!(add_rule.matches_producer(materialize));
    assert!(!add_rule.matches_producer(add_kind));

    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let keys = environment.allocation_constraint_keys();
    assert_eq!(
        add_rule.immediate_constraint_key(&keys),
        Some(keys.add_i64_immediate)
    );
    assert_eq!(
        subtract_rule.immediate_constraint_key(&keys),
        Some(keys.subtract_i64_immediate)
    );
    assert_eq!(
        compare_rule.immediate_constraint_key(&keys),
        Some(keys.compare_i64_immediate)
    );
    assert_eq!(indexed_rule.immediate_constraint_key(&keys), keys.load8);
    assert_eq!(
        copy_rule.immediate_constraint_key(&keys),
        Some(keys.materialize_i64)
    );
    assert_eq!(
        address_offset_rule.immediate_constraint_key(&keys),
        keys.address_offset
    );
}

#[test]
fn declared_unit_effects_admit_the_real_immediate_rows() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        // A real physical unit from the target's condition-state view.
        let unit = environment
            .constraint(keys.compare_i64_immediate)
            .unwrap()
            .implicit_defs[0];

        for rule in [
            SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12,
            SelectedInstructionPairRule::EXACT_ADD_LEFT_IMMEDIATE_U12,
            SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12,
            SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12,
            SelectedInstructionPairRule::LOAD8_INDEXED_U12,
            SelectedInstructionPairRule::COPY_LITERAL_FOLD,
            SelectedInstructionPairRule::BYTE_VIEW_ADDRESS_OFFSET_U12,
        ] {
            let row = environment
                .constraint(rule.immediate_constraint_key(&keys).unwrap())
                .unwrap();
            assert!(rule.unit_effects().admits_row_units(row));
            assert!(
                row.operands
                    .iter()
                    .all(|operand| rule.unit_effects().admits_operand(operand))
            );

            // Unit traffic beyond the declared result channel fails the
            // declared contract: implicit uses, clobbers, and operand unit
            // bindings each reject.
            let mut with_use = row.clone();
            with_use.implicit_uses.push(unit);
            assert!(!rule.unit_effects().admits_row_units(&with_use));
            let mut with_clobber = row.clone();
            with_clobber.clobbers.push(unit);
            assert!(!rule.unit_effects().admits_row_units(&with_clobber));
            let mut decorated = row.clone();
            decorated.operands[0].early_clobber = true;
            assert!(!rule.unit_effects().admits_operand(&decorated.operands[0]));
        }
    }
}

#[test]
fn declared_machine_effects_admit_the_real_catalog_declarations() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let catalog = validated_machine_effect_catalog(target, environment.constraints()).unwrap();
        let declaration = |semantic: MachineSemanticKind| {
            let constraint = keys
                .for_semantic(semantic)
                .expect("the selected key inventory binds the semantic");
            let mut matches = catalog.catalog().declarations.iter().filter(|declaration| {
                declaration.semantic == semantic && declaration.constraint == constraint
            });
            let declaration = matches.next().expect("the catalog declares the form");
            assert!(matches.next().is_none(), "exactly one declaration binds it");
            declaration
        };

        // Every landed isolated pair admits its own triple of real
        // declarations on both targets, whatever flag traffic the target's
        // consumer row actually carries.
        for rule in [
            SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12,
            SelectedInstructionPairRule::EXACT_ADD_LEFT_IMMEDIATE_U12,
            SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12,
            SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12,
        ]
        .into_iter()
        .chain(SelectedInstructionPairRule::EXTENSION_LITERAL_FOLDS)
        .chain([
            SelectedInstructionPairRule::COPY_LITERAL_FOLD,
            SelectedInstructionPairRule::BYTE_VIEW_ADDRESS_OFFSET_U12,
        ]) {
            assert_eq!(rule.machine_effects(), PairMachineEffects::Isolated);
            let producer = declaration(rule.producer());
            let consumer = declaration(rule.consumer());
            let rewritten = declaration(rule.rewritten());
            assert!(
                rule.machine_effects().admits_producer(producer),
                "{rule:?} producer on {target:?}"
            );
            assert!(
                rule.machine_effects().admits_consumer(consumer, rewritten),
                "{rule:?} consumer on {target:?}"
            );
            assert!(
                rule.machine_effects().admits_rewritten(rewritten),
                "{rule:?} rewritten on {target:?}"
            );
        }

        // The indexed byte-load pair admits its own triple on both targets:
        // an isolated producer, the indexed pointer read, and the
        // direct-offset read.
        {
            let rule = SelectedInstructionPairRule::LOAD8_INDEXED_U12;
            let producer = declaration(rule.producer());
            let consumer = declaration(rule.consumer());
            let rewritten = declaration(rule.rewritten());
            assert!(
                rule.machine_effects().admits_producer(producer),
                "{rule:?} producer on {target:?}"
            );
            assert!(
                rule.machine_effects().admits_consumer(consumer, rewritten),
                "{rule:?} consumer on {target:?}"
            );
            assert!(
                rule.machine_effects().admits_rewritten(rewritten),
                "{rule:?} rewritten on {target:?}"
            );
            // The relationship is directional: the direct-offset form is not
            // an indexed read, so swapping consumer and rewritten cannot
            // satisfy the fold.
            assert!(
                !rule.machine_effects().admits_consumer(rewritten, consumer),
                "{rule:?} swapped roles on {target:?}"
            );
        }

        // Declarations carrying memory traffic cannot take any role in an
        // isolated pair on either target.
        for semantic in [MachineSemanticKind::Load64, MachineSemanticKind::Store64] {
            let memory_bound = declaration(semantic);
            let isolated = declaration(MachineSemanticKind::MaterializeI64);
            assert!(!PairMachineEffects::Isolated.admits_producer(memory_bound));
            assert!(!PairMachineEffects::Isolated.admits_consumer(memory_bound, isolated));
            assert!(!PairMachineEffects::Isolated.admits_rewritten(memory_bound));
        }
        // A branching form carries control-flow, barrier, and trap surface.
        let jump = declaration(MachineSemanticKind::Jump);
        let isolated = declaration(MachineSemanticKind::MaterializeI64);
        assert!(!PairMachineEffects::Isolated.admits_producer(jump));
        assert!(!PairMachineEffects::Isolated.admits_consumer(jump, isolated));
        assert!(!PairMachineEffects::Isolated.admits_rewritten(jump));
        // A flag-consuming materialization is isolated outside its units but
        // declares implicit unit uses — an implicit use the rewritten form
        // does not carry cannot be dropped silently.
        let flag_consuming = declaration(MachineSemanticKind::MaterializeBooleanEqual);
        assert!(!PairMachineEffects::Isolated.admits_producer(flag_consuming));
        assert!(!PairMachineEffects::Isolated.admits_consumer(flag_consuming, isolated));
        assert!(!PairMachineEffects::Isolated.admits_rewritten(flag_consuming));
        // A flag-defining consumer cannot feed a scalar-result rewrite: its
        // condition-state definition would not stay defined.
        let compare = declaration(MachineSemanticKind::CompareI64);
        let add_immediate = declaration(MachineSemanticKind::ExactAddI64Immediate);
        assert!(!PairMachineEffects::Isolated.admits_consumer(compare, add_immediate));
    }
}

#[test]
fn extension_elimination_rules_fold_unary_consumers_to_materializations() {
    let extension = SELECTED_LOWERING_RULE_CATALOG[3];
    let pairs = extension.payload().pairs();
    assert_eq!(
        pairs,
        SelectedInstructionPairRule::EXTENSION_LITERAL_FOLDS.as_slice()
    );
    let consumers = pairs.iter().map(|rule| rule.consumer()).collect::<Vec<_>>();
    assert_eq!(
        consumers,
        vec![
            MachineSemanticKind::ZeroExtendU8,
            MachineSemanticKind::ZeroExtendU16,
            MachineSemanticKind::ZeroExtendU32,
            MachineSemanticKind::SignExtendI8,
            MachineSemanticKind::SignExtendI16,
            MachineSemanticKind::SignExtendI32,
        ]
    );
    for rule in pairs {
        assert_eq!(rule.producer(), MachineSemanticKind::MaterializeI64);
        assert_eq!(rule.rewritten(), MachineSemanticKind::MaterializeI64);
        assert_eq!(rule.operand_shape(), PairOperandShape::UnaryLiteral);
        assert_eq!(rule.victim_operand(), 0);
        assert_eq!(rule.result(), PairResultDisposition::ScalarRegister);
        // Extension-folded constants always encode; no immediate bound applies.
        assert_eq!(rule.immediate_limit(), u64::MAX);
        assert!(rule.admits_immediate(u64::MAX));
    }

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        for rule in pairs {
            // Every extension fold rewrites into the target's materialize row.
            assert_eq!(
                rule.immediate_constraint_key(&keys),
                Some(keys.materialize_i64)
            );
            let row = environment.constraint(keys.materialize_i64).unwrap();
            assert_eq!(row.operands.len(), 1);
            assert_eq!(row.operands[0].access, RegisterOperandAccess::Def);
        }
    }

    let zero_u8 = SelectedInstructionPairRule::ZERO_EXTEND_U8_LITERAL_FOLD;
    assert_eq!(zero_u8.fold_immediate(0x1FF), Some(0xFF));
    assert_eq!(zero_u8.fold_immediate(u64::MAX), Some(0xFF));
    let sign_i8 = SelectedInstructionPairRule::SIGN_EXTEND_I8_LITERAL_FOLD;
    assert_eq!(sign_i8.fold_immediate(0x80), Some(u64::MAX - 0x7F));
    assert_eq!(sign_i8.fold_immediate(0x7F), Some(0x7F));
    let sign_i16 = SelectedInstructionPairRule::SIGN_EXTEND_I16_LITERAL_FOLD;
    assert_eq!(sign_i16.fold_immediate(0x8000), Some(u64::MAX - 0x7FFF));
    let sign_i32 = SelectedInstructionPairRule::SIGN_EXTEND_I32_LITERAL_FOLD;
    assert_eq!(
        sign_i32.fold_immediate(0x8000_0000),
        Some(u64::MAX - 0x7FFF_FFFF)
    );

    let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let i8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap());
    let i64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    // Zero extension into an unsigned result materializes the masked value.
    assert_eq!(
        zero_u8.rewrite_consumer(SelectedInstructionKind::ZeroExtendU8, 0xFF, Some(u8_type)),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0xFF),
        })
    );
    // Sign extension into a signed result materializes the signed value.
    assert_eq!(
        sign_i8.rewrite_consumer(
            SelectedInstructionKind::SignExtendI8,
            u64::MAX - 0x7F,
            Some(i8_type)
        ),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(-128),
        })
    );
    // The same folded bits under an i64 result admit the full signed value.
    assert_eq!(
        sign_i8.rewrite_consumer(
            SelectedInstructionKind::SignExtendI8,
            u64::MAX - 0x7F,
            Some(i64_type)
        ),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(-128),
        })
    );
    // Unsigned result types keep the folded bits as the unsigned value.
    assert_eq!(
        sign_i8.rewrite_consumer(
            SelectedInstructionKind::SignExtendI8,
            u64::MAX - 0x7F,
            Some(u64_type)
        ),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(u64::MAX - 0x7F)),
        })
    );
    // Result types that cannot admit the folded value reject.
    assert_eq!(
        zero_u8.rewrite_consumer(SelectedInstructionKind::ZeroExtendU8, 0xFF, Some(i8_type)),
        None
    );
    assert_eq!(
        zero_u8.rewrite_consumer(
            SelectedInstructionKind::ZeroExtendU8,
            0xFF,
            Some(ScalarType::Boolean)
        ),
        None
    );
    // Missing or mismatched consumers and scalar evidence reject.
    assert_eq!(
        zero_u8.rewrite_consumer(SelectedInstructionKind::ZeroExtendU16, 0xFF, Some(u8_type)),
        None
    );
    assert_eq!(
        zero_u8.rewrite_consumer(SelectedInstructionKind::ZeroExtendU8, 0xFF, None),
        None
    );
    assert_eq!(
        zero_u8.rewrite_consumer(SelectedInstructionKind::CompareI64, 0xFF, Some(u64_type)),
        None
    );
}

#[test]
fn copy_materialization_rule_folds_the_unary_copy_to_a_materialization() {
    let copy = SELECTED_LOWERING_RULE_CATALOG[5];
    assert_eq!(
        copy.optimization(),
        Optimization::SelectedIncomingLiteralCopyMaterialization
    );
    let pairs = copy.payload().pairs();
    assert_eq!(pairs, &[SelectedInstructionPairRule::COPY_LITERAL_FOLD]);
    let rule = pairs[0];
    assert_eq!(rule.producer(), MachineSemanticKind::MaterializeI64);
    assert_eq!(rule.consumer(), MachineSemanticKind::CopyI64);
    assert_eq!(rule.rewritten(), MachineSemanticKind::MaterializeI64);
    assert_eq!(rule.operand_shape(), PairOperandShape::UnaryLiteral);
    assert_eq!(rule.victim_operand(), 0);
    assert_eq!(rule.result(), PairResultDisposition::ScalarRegister);
    // The copy preserves the full literal: no target immediate bound and no
    // bit folding — the materialized payload is the literal itself.
    assert_eq!(rule.immediate_limit(), u64::MAX);
    assert_eq!(rule.fold_immediate(0), Some(0));
    assert_eq!(rule.fold_immediate(0x1_0000_0001), Some(0x1_0000_0001));
    assert_eq!(rule.fold_immediate(u64::MAX), Some(u64::MAX));

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        // The copy fold rewrites into the target's materialize row.
        assert_eq!(
            rule.immediate_constraint_key(&keys),
            Some(keys.materialize_i64)
        );
    }

    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let i64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    // An unsigned result materializes the literal's unsigned value.
    assert_eq!(
        rule.rewrite_consumer(
            SelectedInstructionKind::CopyI64,
            0x1_0000_0001,
            Some(u64_type)
        ),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0x1_0000_0001),
        })
    );
    // A signed result materializes the literal's two's-complement value.
    assert_eq!(
        rule.rewrite_consumer(SelectedInstructionKind::CopyI64, u64::MAX, Some(i64_type)),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(-1),
        })
    );
    // The copy cannot narrow: a result type too small for the literal, a
    // non-integer result, missing scalar evidence, or a mismatched consumer
    // all reject.
    assert_eq!(
        rule.rewrite_consumer(SelectedInstructionKind::CopyI64, 0x1FF, Some(u8_type)),
        None
    );
    assert_eq!(
        rule.rewrite_consumer(
            SelectedInstructionKind::CopyI64,
            7,
            Some(ScalarType::Boolean)
        ),
        None
    );
    assert_eq!(
        rule.rewrite_consumer(SelectedInstructionKind::CopyI64, 7, None),
        None
    );
    assert_eq!(
        rule.rewrite_consumer(SelectedInstructionKind::ZeroExtendU8, 0xFF, Some(u8_type)),
        None
    );
}
