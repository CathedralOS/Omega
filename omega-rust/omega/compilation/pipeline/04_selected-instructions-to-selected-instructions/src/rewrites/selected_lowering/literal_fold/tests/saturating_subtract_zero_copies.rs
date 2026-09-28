use super::{
    BlockZeroTerminator, assert_classification_does_not_steer, assert_declines,
    staged_saturating_subtract_carrier_inputs, staged_saturating_subtract_inputs,
};
use crate::LiteralFoldPolicy;
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::{SaturatingCarrier, VirtualRegisterId};
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn saturating_subtract_zero_fold_declines_while_a_terminator_reads_the_defined_unit() {
    // On aarch64 the conditional branch implicitly uses `nzcv`, the very
    // unit the saturating subtract defines on every carrier: retiring the
    // definition would leave the branch observing stale flags, so the
    // dead-definitions gate — the producer's `admits_dead_consumer_defs`
    // and the replay's independent `dropped_unit_defs_dead` — refuses the
    // fold on both paths. x86-64 keeps folding under the same terminator
    // because `rflags` is only a clobber there: removing a clobber narrows
    // destruction and no reader can go stale.
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let branch = environment.constraint(keys.conditional_branch).unwrap();
    for key in [
        keys.saturating_subtract_unsigned,
        keys.saturating_subtract_clamped,
    ] {
        assert!(
            branch.implicit_uses.iter().any(|unit| environment
                .constraint(key)
                .unwrap()
                .implicit_defs
                .contains(unit)),
            "the aarch64 branch reads the unit each saturating-subtract row defines"
        );
    }
    for carrier in [SaturatingCarrier::U64, SaturatingCarrier::I32] {
        let inputs = staged_saturating_subtract_carrier_inputs(
            target,
            carrier,
            1,
            BlockZeroTerminator::ConditionalBranch,
        );
        assert_declines(
            &inputs,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
            &format!("{carrier:?} with a live nzcv reader"),
        );
    }
}

#[test]
fn saturating_subtract_zero_fold_ignores_a_classification_claiming_another_operand() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Each fixture carries the literal at `literal_operand`; the
        // recovery classification claims `claimed_operand` instead. The
        // classification no longer nominates, so the claim neither
        // redirects nor suppresses the fold the stream itself admits.
        for (literal_operand, claimed_operand) in [(1u16, 0u16), (1u16, 2u16), (0u16, 1u16)] {
            let mut inputs = staged_saturating_subtract_inputs(
                target,
                literal_operand,
                BlockZeroTerminator::Jump,
            );
            let RecoveryClassification::ImmediateU64RematerializationCandidate {
                future_uses, ..
            } = &mut inputs.recovery.plan.functions[0]
                .classification
                .as_mut()
                .unwrap()
                .classification
            else {
                unreachable!()
            };
            future_uses[0].operand = claimed_operand;
            assert_classification_does_not_steer(
                &inputs,
                &environment,
                LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
                &format!(
                    "literal at operand {literal_operand} claiming {claimed_operand} on {target:?}"
                ),
            );
        }
    }
}

#[test]
fn saturating_subtract_zero_fold_declines_a_clamped_consumer_whose_scratch_is_not_custodied() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The scratch-defs grammar drops the bound scratch `Def` under
    // occurrence-free custody: the operand list must keep it a `Def`,
    // and its register must occur nowhere else in the function. A `Use`
    // at the tail position has no droppable role; a tail `Def` naming a
    // register another operand position already carries — here the
    // surviving `Use`'s register — leaves a surviving read of a
    // definition the rewrite would stop making.
    for mutation in 0..2 {
        let mut inputs = staged_saturating_subtract_carrier_inputs(
            target,
            SaturatingCarrier::I32,
            1,
            BlockZeroTerminator::Jump,
        );
        let mut plan = inputs.selected.transformed().clone();
        let consumer = &mut plan.functions[0].blocks[0].instructions[1];
        match mutation {
            0 => consumer.operands[3].access = RegisterOperandAccess::Use,
            _ => consumer.operands[3].virtual_register = VirtualRegisterId(0),
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        inputs.selected = selected;
        assert_declines(
            &inputs,
            &environment,
            LiteralFoldPolicy::SATURATING_SUBTRACT_ZERO_V1,
            &format!("mutation {mutation}"),
        );
    }
}
