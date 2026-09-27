use super::{
    assert_budget_is_enforced, budget, fold_with, staged_byte_view_address_backing_inputs,
    staged_byte_view_address_inputs, staged_divide_inputs, staged_load8_indexed_inputs,
};
use crate::analyses::validated_machine_effect_catalog;
use crate::rewrites::{fold_selected_incoming_literal, validate_literal_fold};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use register_environment::baseline_target_register_environment;
use register_homes::RecoveryClassification;
use register_model::RegisterOperandAccess;
use selected_instructions::SelectedInstructionKind;
use semantic_vocabulary::IntegerValue;
use std::sync::Arc;
use target::NativeTarget;

#[test]
fn load8_indexed_fold_reports_and_enforces_its_measured_work() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = staged_load8_indexed_inputs(target, 5);
        assert_budget_is_enforced(&inputs, &environment, LiteralFoldPolicy::LOAD8_INDEXED_V1);
    }
}

#[test]
fn byte_view_address_fold_admits_the_widest_encodable_byte_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // 4095 is the widest offset the shared 12-bit byte-offset bound
        // admits — the boundary the unencodable 4096 sits just past.
        let inputs = staged_byte_view_address_inputs(target, 4095);
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .expect("the boundary offset folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 4095 }
        );
    }
}

#[test]
fn byte_view_address_backing_fold_admits_the_widest_encodable_backing_literal() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // 4095 is the widest offset the shared 12-bit byte-offset bound
        // admits — the boundary the unencodable 4096 sits just past.
        let inputs = staged_byte_view_address_backing_inputs(target, 4095);
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .expect("the boundary backing literal folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 4095);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 4095 }
        );
        // A zero backing literal folds too: the projection computes the
        // surviving offset register's own address, so the rewritten form
        // carries byte offset zero.
        let inputs = staged_byte_view_address_backing_inputs(target, 0);
        let result = fold_with(
            &inputs,
            &environment,
            LiteralFoldPolicy::BYTE_VIEW_ADDRESS_V1,
        )
        .expect("the zero backing literal folds");
        assert_eq!(result.receipt().applied_count(), 1);
        assert_eq!(result.plan().functions[0].action.unwrap().immediate, 0);
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::AddressOffset { byte_offset: 0 }
        );
    }
}

#[test]
fn divide_fold_rejects_a_non_unit_divisor() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let effect_catalog =
            validated_machine_effect_catalog(environment.target(), environment.constraints())
                .unwrap();
        let inputs = staged_divide_inputs(target);
        let auxiliary_uses = environment
            .constraint(keys.divide_u64)
            .unwrap()
            .operands
            .len()
            - 3;
        let mut plan = inputs.selected.transformed().clone();
        // The literal is the divisor only under the identity fold: a divisor
        // of two is a different computation both the producer's declared
        // bound and the replay's re-derived grammar reject.
        plan.functions[0].blocks[0].instructions[auxiliary_uses].kind =
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(2),
            };
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);
        let mut recovery = inputs.recovery.clone();
        let classification = recovery.plan.functions[0].classification.as_mut().unwrap();
        let RecoveryClassification::ImmediateU64RematerializationCandidate { value, .. } =
            &mut classification.classification
        else {
            panic!("staged classification is the immediate candidate");
        };
        *value = IntegerValue::Unsigned(2);

        assert_eq!(
            fold_selected_incoming_literal(
                &selected,
                &inputs.ranges,
                &inputs.legality,
                &inputs.spill_choices,
                &recovery,
                &inputs.availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
                &effect_catalog,
                LiteralFoldPolicy::EXACT_DIVIDE_V1,
                budget(),
            ),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?}"
        );
        assert_eq!(
            validate_literal_fold(
                &selected,
                &inputs.ranges,
                &inputs.legality,
                &inputs.spill_choices,
                &recovery,
                &inputs.availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
                &effect_catalog,
                inputs.selected.plan().clone(),
            ),
            Err(LiteralFoldError::UnsupportedImmediate { function: 0 }),
            "{target:?} replay"
        );
    }
}

#[test]
fn divide_fold_rejects_an_auxiliary_operand_without_zero_custody() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.allocation_constraint_keys();
    let effect_catalog =
        validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap();
    let divide_operands = environment
        .constraint(keys.divide_u64)
        .unwrap()
        .operands
        .len();
    assert_eq!(
        divide_operands, 4,
        "the x86-64 divide carries one auxiliary use"
    );

    // The dropped operand is inert only when its register is defined solely
    // by zero materializations: a nonzero materialization, a non-materialize
    // definition, and a `Def`-access auxiliary operand each reject — the
    // producer and the independent replay alike.
    for mutation in 0..3 {
        let inputs = staged_divide_inputs(target);
        let mut plan = inputs.selected.transformed().clone();
        match mutation {
            // The high-half scratch materializes seven, not zero.
            0 => {
                plan.functions[0].blocks[0].instructions[0].kind =
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(7),
                    };
            }
            // The scratch register is defined by a copy, not a zero
            // materialization at all.
            1 => {
                plan.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::CopyI64;
            }
            // The auxiliary operand defines a register rather than reading
            // the proven-zero scratch.
            _ => {
                plan.functions[0].blocks[0].instructions[2].operands[3].access =
                    RegisterOperandAccess::Def;
            }
        }
        let mut selected = inputs.selected.clone();
        selected.transformed = Arc::new(plan);

        assert_eq!(
            fold_selected_incoming_literal(
                &selected,
                &inputs.ranges,
                &inputs.legality,
                &inputs.spill_choices,
                &inputs.recovery,
                &inputs.availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
                &effect_catalog,
                LiteralFoldPolicy::EXACT_DIVIDE_V1,
                budget(),
            ),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation}"
        );
        assert_eq!(
            validate_literal_fold(
                &selected,
                &inputs.ranges,
                &inputs.legality,
                &inputs.spill_choices,
                &inputs.recovery,
                &inputs.availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
                &effect_catalog,
                inputs.selected.plan().clone(),
            ),
            Err(LiteralFoldError::ConsumerMismatch { function: 0 }),
            "mutation {mutation} replay"
        );
    }
}
