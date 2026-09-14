use optimization_core::{
    AcceptedObligationFactIdentity, Optimization, OptimizationExecutionPhase,
    OptimizationSelections,
};
use register_environment::baseline_target_register_environment;
use selected_instructions::{MachineSemanticKind, SelectedInstructionKind};
use semantic_vocabulary::{IntegerValue, ObligationId};
use target::NativeTarget;

use super::{
    LiteralFoldPolicy, ORDERED_SELECTED_LOWERING_RULES, PairResultDisposition, PairUnitEffects,
    SELECTED_LOWERING_RULE_CATALOG, SelectedInstructionPairRule, enabled_pair_rules,
    resolve_selected_lowering_rules,
};
use crate::RegisterAllocationRuleTargetApplicability;

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
}

#[test]
fn catalog_rows_declare_symbolic_instruction_pairs() {
    let [add, subtract, compare] = SELECTED_LOWERING_RULE_CATALOG;
    for entry in [add, subtract, compare] {
        let pair = entry.payload().pair();
        assert_eq!(pair.producer(), MachineSemanticKind::MaterializeI64);
        assert_eq!(pair.immediate_limit(), 4095);
        assert!(pair.admits_immediate(4095));
        assert!(!pair.admits_immediate(4096));
    }
    let add_rule = add.payload().pair();
    let subtract_rule = subtract.payload().pair();
    assert_eq!(
        add_rule,
        SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12
    );
    assert_eq!(add_rule.consumer(), MachineSemanticKind::ExactAddI64);
    assert_eq!(
        add_rule.rewritten(),
        MachineSemanticKind::ExactAddI64Immediate
    );
    assert_eq!(add_rule.result(), PairResultDisposition::ScalarRegister);
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

    let compare_rule = compare.payload().pair();
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
        compare_rule.rewrite_consumer(SelectedInstructionKind::CompareI64, 12),
        Some(SelectedInstructionKind::CompareI64Immediate {
            immediate: IntegerValue::Unsigned(12),
        })
    );

    // Every landed rule's rewrite is unit-effect isolated: no implicit unit
    // uses or clobbers and no operand unit bindings beyond the declared
    // result channel.
    for entry in [add, subtract, compare] {
        assert_eq!(
            entry.payload().pair().unit_effects(),
            PairUnitEffects::Isolated
        );
    }

    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::EXACT_ADD_V1).collect::<Vec<_>>(),
        vec![SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12]
    );
    let union = LiteralFoldPolicy::EXACT_ADD_V1.union(LiteralFoldPolicy::EXACT_SUBTRACT_V1);
    assert_eq!(
        enabled_pair_rules(union).collect::<Vec<_>>(),
        vec![
            SelectedInstructionPairRule::EXACT_ADD_IMMEDIATE_U12,
            SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12,
        ]
    );
    assert_eq!(
        enabled_pair_rules(LiteralFoldPolicy::COMPARE_V1).collect::<Vec<_>>(),
        vec![SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12]
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
    assert_eq!(add_rule.rewrite_consumer(subtract_kind, 12), None);
    assert_eq!(
        add_rule.rewrite_consumer(add_kind, 12),
        Some(SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(12),
            obligation,
            accepted_fact,
        })
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
            SelectedInstructionPairRule::EXACT_SUBTRACT_IMMEDIATE_U12,
            SelectedInstructionPairRule::COMPARE_IMMEDIATE_U12,
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
