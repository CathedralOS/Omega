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
    LiteralFoldPolicy, ORDERED_SELECTED_LOWERING_RULES, PairImmediateBound, PairMachineEffects,
    PairOperandShape, PairResultDisposition, PairUnitEffects, SELECTED_LOWERING_RULE_CATALOG,
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
    assert!(policy.enables_exact_divide());
    assert!(policy.enables_wrapping_remainder());
    assert!(policy.enables_bitwise_and_zero());
    assert!(policy.enables_bitwise_xor_zero());
    assert!(policy.enables_wrapping_add_zero());
    assert!(policy.enables_bitwise_and_ones());
    assert!(policy.enables_wrapping_remainder_zero());
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
        divide,
        remainder,
        and_zero,
        xor_zero,
        wrapping_add_zero,
        and_ones,
        remainder_zero,
    ] = SELECTED_LOWERING_RULE_CATALOG;
    let obligation = ObligationId::new(7).unwrap();
    let accepted_fact = AcceptedObligationFactIdentity::from_bytes([9; 32]);
    for entry in [subtract, compare] {
        let &[pair] = entry.payload().pairs() else {
            panic!("the subtract and compare families each declare one pair rule")
        };
        assert_eq!(pair.producer(), MachineSemanticKind::MaterializeI64);
        assert_eq!(pair.immediate_bound(), PairImmediateBound::Encoding(4095));
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
        assert_eq!(pair.immediate_bound(), PairImmediateBound::Encoding(4095));
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
    assert_eq!(
        indexed_rule.immediate_bound(),
        PairImmediateBound::Encoding(4095)
    );
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
    assert_eq!(
        copy_rule.immediate_bound(),
        PairImmediateBound::Encoding(u64::MAX)
    );

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
    assert_eq!(
        address_offset_rule.immediate_bound(),
        PairImmediateBound::Encoding(4095)
    );
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

    // The divide-identity family declares the first trap-carrying
    // machine-effect relationship: a divisor literal of exactly one folds
    // `ExactDivideU64` into a `CopyI64` of the dividend, discharging the
    // divide's encoded architectural fault, admitting the register pins the
    // pinned-operand realization requires, and dropping the consumer's
    // zeroed auxiliary `Use` operands.
    let &[divide_rule] = divide.payload().pairs() else {
        panic!("the divide-identity family declares one pair rule")
    };
    assert_eq!(
        divide.optimization(),
        Optimization::SelectedIncomingExactDivideIdentityCopy
    );
    assert_eq!(
        divide_rule,
        SelectedInstructionPairRule::EXACT_DIVIDE_ONE_COPY
    );
    assert_eq!(divide_rule.producer(), MachineSemanticKind::MaterializeI64);
    assert_eq!(divide_rule.consumer(), MachineSemanticKind::ExactDivideU64);
    assert_eq!(divide_rule.rewritten(), MachineSemanticKind::CopyI64);
    assert_eq!(
        divide_rule.immediate_bound(),
        PairImmediateBound::Exactly(1)
    );
    assert!(divide_rule.admits_immediate(1));
    assert!(!divide_rule.admits_immediate(0));
    assert!(!divide_rule.admits_immediate(2));
    assert!(!divide_rule.admits_immediate(u64::MAX));
    assert_eq!(divide_rule.fold_immediate(1), Some(1));
    assert_eq!(
        divide_rule.operand_shape(),
        PairOperandShape::BinaryRightLiteralAuxiliaryUses
    );
    assert_eq!(divide_rule.victim_operand(), 1);
    assert_eq!(divide_rule.result(), PairResultDisposition::ScalarRegister);
    assert_eq!(
        divide_rule.unit_effects(),
        PairUnitEffects::BoundConsumerOperands
    );
    assert_eq!(
        divide_rule.machine_effects(),
        PairMachineEffects::FaultDischargedByLiteral
    );
    assert_eq!(
        divide_rule.rewrite_consumer(
            SelectedInstructionKind::ExactDivideU64 {
                obligation,
                accepted_fact,
            },
            1,
            None
        ),
        Some(SelectedInstructionKind::CopyI64)
    );
    assert_eq!(
        divide_rule.rewrite_consumer(SelectedInstructionKind::CopyI64, 1, None),
        None
    );

    // The remainder-identity family declares the second trap-carrying
    // machine-effect relationship: a divisor literal of exactly one folds
    // `WrappingRemainderI64` into a `MaterializeI64` of the constant zero —
    // a remainder by one is always zero — discharging the remainder's
    // encoded architectural fault, admitting the register pins and
    // early-clobber scratch marks a pinned-scratch realization requires,
    // and dropping the consumer's dividend `Use` and dead scratch `Def`
    // operands.
    let &[remainder_rule] = remainder.payload().pairs() else {
        panic!("the remainder-identity family declares one pair rule")
    };
    assert_eq!(
        remainder.optimization(),
        Optimization::SelectedIncomingWrappingRemainderOneZeroMaterialization
    );
    assert_eq!(
        remainder_rule,
        SelectedInstructionPairRule::WRAPPING_REMAINDER_ONE_MATERIALIZE
    );
    assert_eq!(
        remainder_rule.producer(),
        MachineSemanticKind::MaterializeI64
    );
    assert_eq!(
        remainder_rule.consumer(),
        MachineSemanticKind::WrappingRemainderI64
    );
    assert_eq!(
        remainder_rule.rewritten(),
        MachineSemanticKind::MaterializeI64
    );
    assert_eq!(
        remainder_rule.immediate_bound(),
        PairImmediateBound::Exactly(1)
    );
    assert!(remainder_rule.admits_immediate(1));
    assert!(!remainder_rule.admits_immediate(0));
    assert!(!remainder_rule.admits_immediate(2));
    assert!(!remainder_rule.admits_immediate(u64::MAX));
    assert_eq!(remainder_rule.fold_immediate(1), Some(0));
    assert_eq!(
        remainder_rule.operand_shape(),
        PairOperandShape::BinaryRightLiteralConstantResult
    );
    assert_eq!(remainder_rule.victim_operand(), 1);
    assert_eq!(
        remainder_rule.result(),
        PairResultDisposition::ScalarRegister
    );
    assert_eq!(
        remainder_rule.unit_effects(),
        PairUnitEffects::BoundEarlyClobberConsumerOperands
    );
    assert_eq!(
        remainder_rule.machine_effects(),
        PairMachineEffects::FaultDischargedByLiteral
    );
    let u64_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let i64_scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    let remainder_kind = SelectedInstructionKind::WrappingRemainderI64 {
        obligation,
        accepted_fact,
    };
    assert_eq!(
        remainder_rule.rewrite_consumer(remainder_kind, 0, Some(u64_scalar)),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        })
    );
    assert_eq!(
        remainder_rule.rewrite_consumer(remainder_kind, 0, Some(i64_scalar)),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(0),
        })
    );
    assert_eq!(
        remainder_rule.rewrite_consumer(SelectedInstructionKind::CopyI64, 0, Some(u64_scalar)),
        None
    );
    assert_eq!(
        remainder_rule.rewrite_consumer(remainder_kind, 0, None),
        None
    );

    // The remainder zero-dividend family declares the third trap-carrying
    // machine-effect relationship: a dividend literal of exactly zero folds
    // `WrappingRemainderI64` into a `MaterializeI64` of the constant zero —
    // a remainder of a zero dividend is always zero — under
    // `FaultDischargedByObligation`: the folded literal does not discharge
    // the remainder's encoded architectural fault; the nonzero-divisor
    // obligation the kind carries does. The family shares the divisor-one
    // family's consumer kind and rewritten form; the folded literal's
    // operand position keeps the grammars disjoint.
    let &[remainder_zero_rule] = remainder_zero.payload().pairs() else {
        panic!("the remainder zero-dividend family declares one pair rule")
    };
    assert_eq!(
        remainder_zero.optimization(),
        Optimization::SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization
    );
    assert_eq!(
        remainder_zero_rule,
        SelectedInstructionPairRule::WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE
    );
    assert_eq!(
        remainder_zero_rule.producer(),
        MachineSemanticKind::MaterializeI64
    );
    assert_eq!(
        remainder_zero_rule.consumer(),
        MachineSemanticKind::WrappingRemainderI64
    );
    assert_eq!(
        remainder_zero_rule.rewritten(),
        MachineSemanticKind::MaterializeI64
    );
    assert_eq!(
        remainder_zero_rule.immediate_bound(),
        PairImmediateBound::Exactly(0)
    );
    assert!(remainder_zero_rule.admits_immediate(0));
    assert!(!remainder_zero_rule.admits_immediate(1));
    assert!(!remainder_zero_rule.admits_immediate(2));
    assert!(!remainder_zero_rule.admits_immediate(u64::MAX));
    assert_eq!(remainder_zero_rule.fold_immediate(0), Some(0));
    assert_eq!(
        remainder_zero_rule.operand_shape(),
        PairOperandShape::BinaryLeftLiteralConstantResult
    );
    assert_eq!(remainder_zero_rule.victim_operand(), 0);
    assert_ne!(
        remainder_zero_rule.victim_operand(),
        remainder_rule.victim_operand()
    );
    assert_eq!(
        remainder_zero_rule.result(),
        PairResultDisposition::ScalarRegister
    );
    assert_eq!(
        remainder_zero_rule.unit_effects(),
        PairUnitEffects::BoundEarlyClobberConsumerOperands
    );
    assert_eq!(
        remainder_zero_rule.machine_effects(),
        PairMachineEffects::FaultDischargedByObligation
    );
    assert_eq!(
        remainder_zero_rule.rewrite_consumer(remainder_kind, 0, Some(u64_scalar)),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        })
    );
    assert_eq!(
        remainder_zero_rule.rewrite_consumer(remainder_kind, 0, Some(i64_scalar)),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(0),
        })
    );
    // The consumer guard binds the rule to its own consumer kind: the
    // zero-dividend remainder rule never rewrites a bitwise-and.
    assert_eq!(
        remainder_zero_rule.rewrite_consumer(
            SelectedInstructionKind::BitwiseAndI64,
            0,
            Some(u64_scalar)
        ),
        None
    );
    assert_eq!(
        remainder_zero_rule.rewrite_consumer(remainder_kind, 0, None),
        None
    );

    // The bitwise-and annihilator family declares one pair per `Use`
    // position: a literal of exactly zero folds `BitwiseAndI64` into a
    // `MaterializeI64` of the constant zero — `x & 0` and `0 & x` are both
    // zero — dropping the other `Use` and every dead scratch `Def` operand.
    // Both grammars rewrite through the materialize row the same way; the
    // pair disambiguates by which `Use` position the folded literal
    // occupies, and the left grammar attests no commutation — the
    // operand-0 literal alone fixes the result.
    let &[and_zero_rule, and_zero_left_rule] = and_zero.payload().pairs() else {
        panic!("the and-zero family declares one pair per operand grammar")
    };
    assert_eq!(
        and_zero.optimization(),
        Optimization::SelectedIncomingBitwiseAndZeroMaterialization
    );
    assert_eq!(
        and_zero_rule,
        SelectedInstructionPairRule::BITWISE_AND_ZERO_MATERIALIZE
    );
    assert_eq!(
        and_zero_left_rule,
        SelectedInstructionPairRule::BITWISE_AND_ZERO_LEFT_MATERIALIZE
    );
    for pair in [and_zero_rule, and_zero_left_rule] {
        assert_eq!(pair.producer(), MachineSemanticKind::MaterializeI64);
        assert_eq!(pair.consumer(), MachineSemanticKind::BitwiseAndI64);
        assert_eq!(pair.rewritten(), MachineSemanticKind::MaterializeI64);
        assert_eq!(pair.immediate_bound(), PairImmediateBound::Exactly(0));
        assert!(pair.admits_immediate(0));
        assert!(!pair.admits_immediate(1));
        assert!(!pair.admits_immediate(u64::MAX));
        assert_eq!(pair.fold_immediate(0), Some(0));
        assert_eq!(pair.result(), PairResultDisposition::ScalarRegister);
        assert_eq!(pair.unit_effects(), PairUnitEffects::Isolated);
        assert_eq!(pair.machine_effects(), PairMachineEffects::Isolated);
    }
    assert_eq!(
        and_zero_rule.operand_shape(),
        PairOperandShape::BinaryRightLiteralConstantResult
    );
    assert_eq!(and_zero_rule.victim_operand(), 1);
    assert_eq!(
        and_zero_left_rule.operand_shape(),
        PairOperandShape::BinaryLeftLiteralConstantResult
    );
    assert_eq!(and_zero_left_rule.victim_operand(), 0);
    let and_kind = SelectedInstructionKind::BitwiseAndI64;
    assert_eq!(
        and_zero_rule.rewrite_consumer(and_kind, 0, Some(u64_scalar)),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        })
    );
    assert_eq!(
        and_zero_left_rule.rewrite_consumer(and_kind, 0, Some(i64_scalar)),
        Some(SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(0),
        })
    );
    assert_eq!(
        and_zero_rule.rewrite_consumer(remainder_kind, 0, Some(u64_scalar)),
        None
    );
    assert_eq!(and_zero_rule.rewrite_consumer(and_kind, 0, None), None);

    // The bitwise-xor identity family declares one pair per `Use`
    // position: a literal of exactly zero folds `BitwiseXorI64` into a
    // `CopyI64` of the surviving `Use` — `x ^ 0` and `0 ^ x` are both `x`.
    // Both grammars rewrite through the same `CopyI64` row the divide fold
    // binds; the pair disambiguates by which `Use` position the folded
    // literal occupies, and the left grammar attests the commutation the
    // surviving-operand binding requires.
    let &[xor_zero_rule, xor_zero_left_rule] = xor_zero.payload().pairs() else {
        panic!("the xor-zero family declares one pair per operand grammar")
    };
    assert_eq!(
        xor_zero.optimization(),
        Optimization::SelectedIncomingBitwiseXorZeroIdentityCopy
    );
    assert_eq!(
        xor_zero_rule,
        SelectedInstructionPairRule::BITWISE_XOR_ZERO_COPY
    );
    assert_eq!(
        xor_zero_left_rule,
        SelectedInstructionPairRule::BITWISE_XOR_ZERO_LEFT_COPY
    );
    for pair in [xor_zero_rule, xor_zero_left_rule] {
        assert_eq!(pair.producer(), MachineSemanticKind::MaterializeI64);
        assert_eq!(pair.consumer(), MachineSemanticKind::BitwiseXorI64);
        assert_eq!(pair.rewritten(), MachineSemanticKind::CopyI64);
        assert_eq!(pair.immediate_bound(), PairImmediateBound::Exactly(0));
        assert!(pair.admits_immediate(0));
        assert!(!pair.admits_immediate(1));
        assert!(!pair.admits_immediate(u64::MAX));
        // The recorded immediate is the folded literal itself — zero —
        // unused by the `CopyI64` rewrite.
        assert_eq!(pair.fold_immediate(0), Some(0));
        assert_eq!(pair.result(), PairResultDisposition::ScalarRegister);
        assert_eq!(pair.unit_effects(), PairUnitEffects::Isolated);
        assert_eq!(pair.machine_effects(), PairMachineEffects::Isolated);
    }
    assert_eq!(
        xor_zero_rule.operand_shape(),
        PairOperandShape::BinaryRightLiteral
    );
    assert_eq!(xor_zero_rule.victim_operand(), 1);
    assert_eq!(
        xor_zero_left_rule.operand_shape(),
        PairOperandShape::BinaryLeftLiteral
    );
    assert_eq!(xor_zero_left_rule.victim_operand(), 0);
    let xor_kind = SelectedInstructionKind::BitwiseXorI64;
    assert_eq!(
        xor_zero_rule.rewrite_consumer(xor_kind, 0, Some(u64_scalar)),
        Some(SelectedInstructionKind::CopyI64)
    );
    assert_eq!(
        xor_zero_left_rule.rewrite_consumer(xor_kind, 0, Some(i64_scalar)),
        Some(SelectedInstructionKind::CopyI64)
    );
    assert_eq!(
        xor_zero_rule.rewrite_consumer(and_kind, 0, Some(u64_scalar)),
        None
    );
    assert_eq!(
        xor_zero_rule.rewrite_consumer(xor_kind, 0, None),
        Some(SelectedInstructionKind::CopyI64)
    );

    // The wrapping-add identity family declares one pair per `Use`
    // position: a literal of exactly zero folds `WrappingAddI64` into a
    // `CopyI64` of the surviving `Use` — `x + 0` and `0 + x` are both `x`
    // modulo 2^64. Both grammars rewrite through the same `CopyI64` row
    // the divide and xor folds bind; the pair disambiguates by which `Use`
    // position the folded literal occupies, and the left grammar attests
    // the commutation the surviving-operand binding requires.
    let &[wrapping_add_zero_rule, wrapping_add_zero_left_rule] =
        wrapping_add_zero.payload().pairs()
    else {
        panic!("the wrapping-add-zero family declares one pair per operand grammar")
    };
    assert_eq!(
        wrapping_add_zero.optimization(),
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy
    );
    assert_eq!(
        wrapping_add_zero_rule,
        SelectedInstructionPairRule::WRAPPING_ADD_ZERO_COPY
    );
    assert_eq!(
        wrapping_add_zero_left_rule,
        SelectedInstructionPairRule::WRAPPING_ADD_ZERO_LEFT_COPY
    );
    for pair in [wrapping_add_zero_rule, wrapping_add_zero_left_rule] {
        assert_eq!(pair.producer(), MachineSemanticKind::MaterializeI64);
        assert_eq!(pair.consumer(), MachineSemanticKind::WrappingAddI64);
        assert_eq!(pair.rewritten(), MachineSemanticKind::CopyI64);
        assert_eq!(pair.immediate_bound(), PairImmediateBound::Exactly(0));
        assert!(pair.admits_immediate(0));
        assert!(!pair.admits_immediate(1));
        assert!(!pair.admits_immediate(u64::MAX));
        // The recorded immediate is the folded literal itself — zero —
        // unused by the `CopyI64` rewrite.
        assert_eq!(pair.fold_immediate(0), Some(0));
        assert_eq!(pair.result(), PairResultDisposition::ScalarRegister);
        assert_eq!(pair.unit_effects(), PairUnitEffects::Isolated);
        assert_eq!(pair.machine_effects(), PairMachineEffects::Isolated);
    }
    assert_eq!(
        wrapping_add_zero_rule.operand_shape(),
        PairOperandShape::BinaryRightLiteral
    );
    assert_eq!(wrapping_add_zero_rule.victim_operand(), 1);
    assert_eq!(
        wrapping_add_zero_left_rule.operand_shape(),
        PairOperandShape::BinaryLeftLiteral
    );
    assert_eq!(wrapping_add_zero_left_rule.victim_operand(), 0);
    let wrapping_add_kind = SelectedInstructionKind::WrappingAddI64;
    assert_eq!(
        wrapping_add_zero_rule.rewrite_consumer(wrapping_add_kind, 0, Some(u64_scalar)),
        Some(SelectedInstructionKind::CopyI64)
    );
    assert_eq!(
        wrapping_add_zero_left_rule.rewrite_consumer(wrapping_add_kind, 0, Some(i64_scalar)),
        Some(SelectedInstructionKind::CopyI64)
    );
    assert_eq!(
        wrapping_add_zero_rule.rewrite_consumer(xor_kind, 0, Some(u64_scalar)),
        None
    );
    assert_eq!(
        wrapping_add_zero_rule.rewrite_consumer(wrapping_add_kind, 0, None),
        Some(SelectedInstructionKind::CopyI64)
    );

    // The bitwise-and identity family declares one pair per `Use`
    // position: the all-ones literal folds `BitwiseAndI64` into a
    // `CopyI64` of the surviving `Use` — `x & MAX` and `MAX & x` are
    // both `x`. The family shares its consumer kind and operand positions
    // with the and-zero annihilator family; the exact literal bound keeps
    // the two grammars disjoint on the literal's value. Both grammars
    // rewrite through the same `CopyI64` row the divide, xor, and
    // wrapping-add folds bind; the pair disambiguates by which `Use`
    // position the folded literal occupies, and the left grammar attests
    // the commutation the surviving-operand binding requires.
    let &[and_ones_rule, and_ones_left_rule] = and_ones.payload().pairs() else {
        panic!("the and-ones family declares one pair per operand grammar")
    };
    assert_eq!(
        and_ones.optimization(),
        Optimization::SelectedIncomingBitwiseAndOnesIdentityCopy
    );
    assert_eq!(
        and_ones_rule,
        SelectedInstructionPairRule::BITWISE_AND_ONES_COPY
    );
    assert_eq!(
        and_ones_left_rule,
        SelectedInstructionPairRule::BITWISE_AND_ONES_LEFT_COPY
    );
    for pair in [and_ones_rule, and_ones_left_rule] {
        assert_eq!(pair.producer(), MachineSemanticKind::MaterializeI64);
        assert_eq!(pair.consumer(), MachineSemanticKind::BitwiseAndI64);
        assert_eq!(pair.rewritten(), MachineSemanticKind::CopyI64);
        assert_eq!(
            pair.immediate_bound(),
            PairImmediateBound::Exactly(u64::MAX)
        );
        assert!(pair.admits_immediate(u64::MAX));
        assert!(!pair.admits_immediate(0));
        assert!(!pair.admits_immediate(1));
        assert!(!pair.admits_immediate(u64::MAX - 1));
        // The recorded immediate is the folded literal itself — all
        // ones — unused by the `CopyI64` rewrite.
        assert_eq!(pair.fold_immediate(u64::MAX), Some(u64::MAX));
        assert_eq!(pair.result(), PairResultDisposition::ScalarRegister);
        assert_eq!(pair.unit_effects(), PairUnitEffects::Isolated);
        assert_eq!(pair.machine_effects(), PairMachineEffects::Isolated);
    }
    // The same consumer kind and `Use` positions the and-zero
    // annihilator grammar covers — the literal bound alone keeps the
    // families disjoint.
    assert_eq!(
        and_ones_rule.victim_operand(),
        and_zero_rule.victim_operand()
    );
    assert_eq!(
        and_ones_left_rule.victim_operand(),
        and_zero_left_rule.victim_operand()
    );
    assert_eq!(
        and_ones_rule.operand_shape(),
        PairOperandShape::BinaryRightLiteral
    );
    assert_eq!(and_ones_rule.victim_operand(), 1);
    assert_eq!(
        and_ones_left_rule.operand_shape(),
        PairOperandShape::BinaryLeftLiteral
    );
    assert_eq!(and_ones_left_rule.victim_operand(), 0);
    assert_eq!(
        and_ones_rule.rewrite_consumer(and_kind, u64::MAX, Some(u64_scalar)),
        Some(SelectedInstructionKind::CopyI64)
    );
    assert_eq!(
        and_ones_left_rule.rewrite_consumer(and_kind, u64::MAX, Some(i64_scalar)),
        Some(SelectedInstructionKind::CopyI64)
    );
    assert_eq!(
        and_ones_rule.rewrite_consumer(xor_kind, u64::MAX, Some(u64_scalar)),
        None
    );
    assert_eq!(
        and_ones_rule.rewrite_consumer(and_kind, u64::MAX, None),
        Some(SelectedInstructionKind::CopyI64)
    );

    // Every landed rule's rewrite but the divide and remainder folds is
    // unit-effect isolated: no implicit unit uses or clobbers and no
    // operand unit bindings beyond the declared result channel. The divide
    // fold deliberately drops the pinned consumer's `fixed_view` bindings;
    // the remainder folds drop the pinned consumer's `fixed_view` pins and
    // `early_clobber` scratch marks.
    for entry in [
        add,
        subtract,
        compare,
        indexed,
        copy,
        address_offset,
        and_zero,
        xor_zero,
        wrapping_add_zero,
        and_ones,
    ] {
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
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::EXACT_DIVIDE_V1).collect::<Vec<_>>(),
        vec![SelectedInstructionPairRule::EXACT_DIVIDE_ONE_COPY]
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::WRAPPING_REMAINDER_V1).collect::<Vec<_>>(),
        vec![SelectedInstructionPairRule::WRAPPING_REMAINDER_ONE_MATERIALIZE]
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::WRAPPING_REMAINDER_ZERO_V1).collect::<Vec<_>>(),
        vec![SelectedInstructionPairRule::WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE]
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::BITWISE_AND_ZERO_V1).collect::<Vec<_>>(),
        SelectedInstructionPairRule::BITWISE_AND_ZERO_FOLDS.to_vec()
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::BITWISE_XOR_ZERO_V1).collect::<Vec<_>>(),
        SelectedInstructionPairRule::BITWISE_XOR_ZERO_COPIES.to_vec()
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::WRAPPING_ADD_ZERO_V1).collect::<Vec<_>>(),
        SelectedInstructionPairRule::WRAPPING_ADD_ZERO_COPIES.to_vec()
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::BITWISE_AND_ONES_V1).collect::<Vec<_>>(),
        SelectedInstructionPairRule::BITWISE_AND_ONES_COPIES.to_vec()
    );
    assert_eq!(enabled_pair_rules(LiteralFoldPolicy::empty()).count(), 0);

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
    assert_eq!(
        divide_rule.immediate_constraint_key(&keys),
        Some(keys.copy_i64)
    );
    assert_eq!(
        remainder_rule.immediate_constraint_key(&keys),
        Some(keys.materialize_i64)
    );
    assert_eq!(
        remainder_zero_rule.immediate_constraint_key(&keys),
        Some(keys.materialize_i64)
    );
    assert_eq!(
        and_zero_rule.immediate_constraint_key(&keys),
        Some(keys.materialize_i64)
    );
    assert_eq!(
        and_zero_left_rule.immediate_constraint_key(&keys),
        Some(keys.materialize_i64)
    );
    assert_eq!(
        xor_zero_rule.immediate_constraint_key(&keys),
        Some(keys.copy_i64)
    );
    assert_eq!(
        xor_zero_left_rule.immediate_constraint_key(&keys),
        Some(keys.copy_i64)
    );
    assert_eq!(
        wrapping_add_zero_rule.immediate_constraint_key(&keys),
        Some(keys.copy_i64)
    );
    assert_eq!(
        wrapping_add_zero_left_rule.immediate_constraint_key(&keys),
        Some(keys.copy_i64)
    );
    assert_eq!(
        and_ones_rule.immediate_constraint_key(&keys),
        Some(keys.copy_i64)
    );
    assert_eq!(
        and_ones_left_rule.immediate_constraint_key(&keys),
        Some(keys.copy_i64)
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
            // The copy row the divide fold rewrites into is itself
            // unit-clean; `BoundConsumerOperands` relaxes only the dropped
            // consumer's operand bindings, not the rewritten row.
            SelectedInstructionPairRule::EXACT_DIVIDE_ONE_COPY,
            // The materialize row the remainder folds rewrite into is
            // likewise unit-clean; `BoundEarlyClobberConsumerOperands`
            // relaxes only the dropped consumer's operand decorations.
            SelectedInstructionPairRule::WRAPPING_REMAINDER_ONE_MATERIALIZE,
            SelectedInstructionPairRule::WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE,
            // Both and-zero grammars rewrite into the same materialize
            // row, which is itself unit-clean.
            SelectedInstructionPairRule::BITWISE_AND_ZERO_MATERIALIZE,
            SelectedInstructionPairRule::BITWISE_AND_ZERO_LEFT_MATERIALIZE,
            // Both xor-zero grammars rewrite into the copy row, which is
            // likewise unit-clean.
            SelectedInstructionPairRule::BITWISE_XOR_ZERO_COPY,
            SelectedInstructionPairRule::BITWISE_XOR_ZERO_LEFT_COPY,
            // Both wrapping-add-zero grammars rewrite into the same copy
            // row, which is likewise unit-clean.
            SelectedInstructionPairRule::WRAPPING_ADD_ZERO_COPY,
            SelectedInstructionPairRule::WRAPPING_ADD_ZERO_LEFT_COPY,
            // Both and-ones grammars rewrite into the same copy row.
            SelectedInstructionPairRule::BITWISE_AND_ONES_COPY,
            SelectedInstructionPairRule::BITWISE_AND_ONES_LEFT_COPY,
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
        ])
        .chain(SelectedInstructionPairRule::BITWISE_AND_ZERO_FOLDS)
        .chain(SelectedInstructionPairRule::BITWISE_XOR_ZERO_COPIES)
        .chain(SelectedInstructionPairRule::WRAPPING_ADD_ZERO_COPIES)
        .chain(SelectedInstructionPairRule::BITWISE_AND_ONES_COPIES)
        {
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

        // The divide-identity pair admits its own triple on both targets:
        // an isolated producer, the possibly-faulting divide, and the
        // isolated copy — x86-64's `div` encodes `MayArchitecturalFaultV1`
        // while aarch64's `udiv` encodes `NeverV1`, and both satisfy the
        // fault-discharging consumer surface.
        {
            let rule = SelectedInstructionPairRule::EXACT_DIVIDE_ONE_COPY;
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
            // A faulting surface the literal does not discharge — memory
            // traffic or a hosted trap — cannot take the consumer role.
            let memory_bound = declaration(MachineSemanticKind::Load64);
            assert!(
                !rule
                    .machine_effects()
                    .admits_consumer(memory_bound, rewritten),
                "{rule:?} memory consumer on {target:?}"
            );
            let control_flow = declaration(MachineSemanticKind::Jump);
            assert!(
                !rule
                    .machine_effects()
                    .admits_consumer(control_flow, rewritten),
                "{rule:?} control-flow consumer on {target:?}"
            );
        }

        // The remainder-identity pair admits its own triple on both
        // targets: an isolated producer, the possibly-faulting remainder,
        // and the isolated materialization — x86-64's `idiv` encodes
        // `MayArchitecturalFaultV1` while aarch64's `udiv`/`msub` encodes
        // `NeverV1`, and both satisfy the fault-discharging consumer
        // surface.
        {
            let rule = SelectedInstructionPairRule::WRAPPING_REMAINDER_ONE_MATERIALIZE;
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
            // A faulting surface the literal does not discharge — memory
            // traffic or a hosted trap — cannot take the consumer role.
            let memory_bound = declaration(MachineSemanticKind::Load64);
            assert!(
                !rule
                    .machine_effects()
                    .admits_consumer(memory_bound, rewritten),
                "{rule:?} memory consumer on {target:?}"
            );
            let control_flow = declaration(MachineSemanticKind::Jump);
            assert!(
                !rule
                    .machine_effects()
                    .admits_consumer(control_flow, rewritten),
                "{rule:?} control-flow consumer on {target:?}"
            );
        }

        // The remainder zero-dividend pair admits its own triple on both
        // targets under the obligation-discharged surface: the same
        // possibly-faulting remainder consumer and isolated materialization
        // the divisor-one fold binds — the declaration-level requirement
        // is the shared fault-discharged shape; the distinguishing
        // obligation custody is instruction-level and the producer and
        // replay check it against the consumer record itself.
        {
            let rule = SelectedInstructionPairRule::WRAPPING_REMAINDER_ZERO_DIVIDEND_MATERIALIZE;
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
            // A faulting surface the obligation does not discharge —
            // memory traffic or a hosted trap — cannot take the consumer
            // role.
            let memory_bound = declaration(MachineSemanticKind::Load64);
            assert!(
                !rule
                    .machine_effects()
                    .admits_consumer(memory_bound, rewritten),
                "{rule:?} memory consumer on {target:?}"
            );
            let control_flow = declaration(MachineSemanticKind::Jump);
            assert!(
                !rule
                    .machine_effects()
                    .admits_consumer(control_flow, rewritten),
                "{rule:?} control-flow consumer on {target:?}"
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
        assert_eq!(
            rule.immediate_bound(),
            PairImmediateBound::Encoding(u64::MAX)
        );
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
    assert_eq!(
        rule.immediate_bound(),
        PairImmediateBound::Encoding(u64::MAX)
    );
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
