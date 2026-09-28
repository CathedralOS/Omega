//! Ordinary literal nomination: the fold scans every `MaterializeI64` in a
//! function instead of reading one pressure-recovery classification row, so
//! several independent literals fold in one function and iteration, a
//! literal no classification names still folds, and a literal whose
//! use/provenance custody the stream does not prove declines while its
//! neighbours fold.

use super::{
    Inputs, assert_deterministic_fixed_point, budget, fold_with, fold_with_budget,
    staged_add_inputs, staged_subtract_inputs, validate,
};
use crate::{LiteralFoldError, LiteralFoldPolicy};
use optimization_core::OptimizationWorkBudget;
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionKind, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{BlockId, IntegerValue, OperationId, ValueId};
use std::sync::Arc;
use target::NativeTarget;

const SECOND_LITERAL: u64 = 9;

/// Extend the staged subtract fixture — `v1 = 5; v2 = v0 - v1` — with an
/// independent second pair in the same block: `v3 = 9; v4 = v2 - v3`. The
/// new literal carries its own single-operation provenance and register
/// origin, and every identifier stays dense: the new records take
/// instructions 2 and 3 and registers 3 and 4, and the two terminators move
/// to instructions 4 and 5.
fn two_subtract_inputs(target: NativeTarget) -> Inputs {
    let mut inputs = staged_subtract_inputs(target);
    let mut plan = inputs.selected.transformed().clone();
    let function = &mut plan.functions[0];
    let block = &mut function.blocks[0];
    let mut literal = block.instructions[0].clone();
    let mut consumer = block.instructions[1].clone();
    let operation = OperationId::new(11).unwrap();
    let value = ValueId::new(21).unwrap();
    literal.id = SelectedInstructionId(2);
    literal.kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(u128::from(SECOND_LITERAL)),
    };
    literal.operands[0].virtual_register = VirtualRegisterId(3);
    literal.provenance.operations = vec![operation];
    literal.provenance.values = vec![value];
    literal.provenance.fuel = vec![FuelSettlement {
        site: PsiProvenance::Operation(operation),
        units: 2,
    }];
    consumer.id = SelectedInstructionId(3);
    consumer.operands[0].virtual_register = VirtualRegisterId(2);
    consumer.operands[1].virtual_register = VirtualRegisterId(3);
    consumer.operands[2].virtual_register = VirtualRegisterId(4);
    consumer.provenance.operations = vec![OperationId::new(12).unwrap()];
    consumer.provenance.values = vec![ValueId::new(22).unwrap()];
    block.instructions.push(literal);
    block.instructions.push(consumer);
    renumber_terminators(function, 4);
    let template = function.virtual_registers[2].clone();
    function.virtual_registers.push(VirtualRegister {
        id: VirtualRegisterId(3),
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(2),
            source_value: value,
        },
        definition_site: Some(ValueDefinitionSite::Node {
            block: BlockId::new(1).unwrap(),
            node: 2,
        }),
        ..template.clone()
    });
    function.virtual_registers.push(VirtualRegister {
        id: VirtualRegisterId(4),
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(3),
            source_value: ValueId::new(22).unwrap(),
        },
        definition_site: Some(ValueDefinitionSite::Node {
            block: BlockId::new(1).unwrap(),
            node: 3,
        }),
        ..template
    });
    inputs.selected.transformed = Arc::new(plan);
    inputs
}

fn renumber_terminators(function: &mut selected_instructions::SelectedFunction, first: u32) {
    for (offset, block) in function.blocks.iter_mut().enumerate() {
        let instruction = match &mut block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
            | SelectedTerminator::Crash { instruction, .. }
            | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
        };
        instruction.id = SelectedInstructionId(first + u32::try_from(offset).unwrap());
    }
}

fn edit_plan(inputs: &mut Inputs, edit: impl FnOnce(&mut selected_instructions::SelectedFunction)) {
    let mut plan = inputs.selected.transformed().clone();
    edit(&mut plan.functions[0]);
    inputs.selected.transformed = Arc::new(plan);
}

#[test]
fn two_independent_literals_fold_in_one_function_and_iteration() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = two_subtract_inputs(target);
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1)
            .unwrap_or_else(|error| panic!("both literals fold on {target:?}: {error:?}"));
        assert_eq!(result.receipt().applied_count(), 2, "{target:?}");
        assert_eq!(result.plan().usage.candidates, 2, "{target:?}");
        assert_eq!(result.plan().usage.commits, 2, "{target:?}");
        assert_eq!(result.plan().usage.iterations, 1, "{target:?}");
        let actions = &result.plan().functions[0].actions;
        assert_eq!(
            actions
                .iter()
                .map(|action| (action.literal_instruction, action.consumer_instruction))
                .collect::<Vec<_>>(),
            vec![
                (SelectedInstructionId(0), SelectedInstructionId(1)),
                (SelectedInstructionId(2), SelectedInstructionId(3)),
            ],
            "actions are recorded in scan order on {target:?}"
        );
        assert_eq!(actions[1].immediate, SECOND_LITERAL);

        // Both literals and both victim registers leave; the survivors
        // redensify over the removed sets together, so the second
        // subtract reads the first one's renumbered result.
        let function = &result.transformed().functions[0];
        let kinds = function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.kind)
            .collect::<Vec<_>>();
        assert!(
            matches!(
                kinds.as_slice(),
                [
                    SelectedInstructionKind::ExactSubtractI64Immediate {
                        immediate: IntegerValue::Unsigned(5),
                        ..
                    },
                    SelectedInstructionKind::ExactSubtractI64Immediate {
                        immediate: IntegerValue::Unsigned(9),
                        ..
                    },
                ]
            ),
            "{kinds:?} on {target:?}"
        );
        assert_eq!(
            function
                .blocks
                .iter()
                .flat_map(|block| block.instructions.iter())
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(0), SelectedInstructionId(1)]
        );
        assert_eq!(function.virtual_registers.len(), 3);
        let second = &function.blocks[0].instructions[1];
        assert_eq!(second.operands[0].virtual_register, VirtualRegisterId(1));
        assert_eq!(second.operands[1].virtual_register, VirtualRegisterId(2));
        assert_eq!(
            function.virtual_registers[2].origin,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(1),
                source_value: ValueId::new(22).unwrap(),
            }
        );

        let replayed = validate(&inputs, &environment, result.plan().clone())
            .unwrap_or_else(|error| panic!("the two-fold plan replays on {target:?}: {error:?}"));
        assert_eq!(replayed.transformed(), result.transformed());
        assert_eq!(replayed.receipt().applied_count(), 2);
    }
}

#[test]
fn literals_no_classification_names_still_fold() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let classified = two_subtract_inputs(target);
        let mut unclassified = classified.clone();
        unclassified.recovery.plan.functions[0].classification = None;
        let with_row = fold_with(
            &classified,
            &environment,
            LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        )
        .unwrap();
        let without_row = fold_with(
            &unclassified,
            &environment,
            LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        )
        .unwrap_or_else(|error| {
            panic!("nomination needs no pressure-recovery row on {target:?}: {error:?}")
        });
        assert_eq!(without_row.receipt().applied_count(), 2);
        assert_eq!(without_row.plan().functions, with_row.plan().functions);
        assert_eq!(without_row.transformed(), with_row.transformed());
        validate(&unclassified, &environment, without_row.plan().clone())
            .expect("the unclassified plan replays");
    }
}

#[test]
fn a_literal_with_a_second_use_declines_while_its_neighbour_folds() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut inputs = two_subtract_inputs(target);
        // The second subtract's minuend now reads the first literal's
        // register: that literal has two uses and must stay materialized,
        // while the second literal's sole use still folds.
        edit_plan(&mut inputs, |function| {
            function.blocks[0].instructions[3].operands[0].virtual_register = VirtualRegisterId(1);
        });
        let result =
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).unwrap();
        let actions = &result.plan().functions[0].actions;
        assert_eq!(actions.len(), 1, "{target:?}");
        assert_eq!(actions[0].literal_instruction, SelectedInstructionId(2));
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[0].kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(5),
            },
            "the multi-use literal stays in the stream on {target:?}"
        );
        validate(&inputs, &environment, result.plan().clone()).expect("the plan replays");

        // A plan claiming the declined literal's fold too is refused.
        let mut forged = result.plan().clone();
        let mut claimed = forged.functions[0].actions[0];
        claimed.literal_instruction = SelectedInstructionId(0);
        claimed.victim = VirtualRegisterId(1);
        claimed.consumer_instruction = SelectedInstructionId(1);
        claimed.surviving = VirtualRegisterId(0);
        claimed.result = Some(VirtualRegisterId(2));
        claimed.immediate = 5;
        forged.functions[0].actions.insert(0, claimed);
        assert_eq!(
            validate(&inputs, &environment, forged).map(|_| ()),
            Err(LiteralFoldError::DecisionMismatch { function: 0 })
        );
    }
}

#[test]
fn a_literal_used_in_another_block_declines_while_its_neighbour_folds() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut inputs = two_subtract_inputs(target);
        // The second subtract moves to the successor block: its literal's
        // only use is no longer in the defining block.
        edit_plan(&mut inputs, |function| {
            let consumer = function.blocks[0].instructions.remove(3);
            function.blocks[1].instructions.push(consumer);
        });
        let result =
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).unwrap();
        let actions = &result.plan().functions[0].actions;
        assert_eq!(actions.len(), 1, "{target:?}");
        assert_eq!(actions[0].literal_instruction, SelectedInstructionId(0));
        validate(&inputs, &environment, result.plan().clone()).expect("the plan replays");
    }
}

#[test]
fn a_literal_carried_across_an_edge_declines() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let mut inputs = two_subtract_inputs(target);
        // The second literal also leaves the block as a successor binding
        // argument: the transport keeps the register live past the fold.
        edit_plan(&mut inputs, |function| {
            let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
                &mut function.blocks[0].terminator
            else {
                panic!("the staged block branches")
            };
            when_nonzero
                .bindings
                .push(selected_instructions::SelectedValueBinding {
                    semantic: abstract_operations::ValueBinding {
                        parameter: ValueId::new(23).unwrap(),
                        argument: ValueId::new(21).unwrap(),
                        scalar_type: function.virtual_registers[3].scalar_type,
                    },
                    transport: selected_instructions::SelectedValueTransport::Registers {
                        argument: VirtualRegisterId(3),
                        parameter: VirtualRegisterId(0),
                    },
                });
        });
        let result =
            fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).unwrap();
        let actions = &result.plan().functions[0].actions;
        assert_eq!(actions.len(), 1, "{target:?}");
        assert_eq!(actions[0].literal_instruction, SelectedInstructionId(0));
        validate(&inputs, &environment, result.plan().clone()).expect("the plan replays");
    }
}

#[test]
fn two_literals_feeding_one_consumer_fold_only_the_first_in_scan_order() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        // `v1 = 5; v2 = v0 + v1` gains a second literal `v3 = 7` that
        // replaces the add's left operand: both literals feed the one
        // commutative consumer. The first in scan order claims it; the
        // second declines and stays materialized as the surviving operand.
        let mut inputs = staged_add_inputs(target, 1);
        edit_plan(&mut inputs, |function| {
            let block = &mut function.blocks[0];
            let mut literal = block.instructions[0].clone();
            let operation = OperationId::new(11).unwrap();
            let value = ValueId::new(21).unwrap();
            literal.id = SelectedInstructionId(2);
            literal.kind = SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(7),
            };
            literal.operands[0].virtual_register = VirtualRegisterId(3);
            literal.provenance.operations = vec![operation];
            literal.provenance.values = vec![value];
            literal.provenance.fuel = vec![FuelSettlement {
                site: PsiProvenance::Operation(operation),
                units: 2,
            }];
            block.instructions.insert(1, literal);
            let consumer = &mut block.instructions[2];
            assert_eq!(consumer.operands[0].access, RegisterOperandAccess::Use);
            consumer.operands[0].virtual_register = VirtualRegisterId(3);
            renumber_terminators(function, 3);
            let template = function.virtual_registers[1].clone();
            function.virtual_registers.push(VirtualRegister {
                id: VirtualRegisterId(3),
                origin: VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(2),
                    source_value: value,
                },
                ..template
            });
        });
        let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_ADD_V1)
            .unwrap_or_else(|error| panic!("the first literal folds on {target:?}: {error:?}"));
        let actions = &result.plan().functions[0].actions;
        assert_eq!(actions.len(), 1, "{target:?}");
        assert_eq!(actions[0].literal_instruction, SelectedInstructionId(0));
        assert_eq!(actions[0].surviving, VirtualRegisterId(3));
        let block = &result.transformed().functions[0].blocks[0];
        assert_eq!(
            block.instructions[0].kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(7),
            },
            "the second literal remains the surviving operand on {target:?}"
        );
        validate(&inputs, &environment, result.plan().clone()).expect("the plan replays");
    }
}

#[test]
fn replay_refuses_every_multi_action_plan_substitution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = two_subtract_inputs(target);
    let result = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1).unwrap();
    for mutation in 0..5 {
        let mut plan = result.plan().clone();
        let expected = match mutation {
            // Dropping either recorded action.
            0 => {
                plan.functions[0].actions.remove(0);
                LiteralFoldError::DecisionMismatch { function: 0 }
            }
            1 => {
                plan.functions[0].actions.pop();
                LiteralFoldError::DecisionMismatch { function: 0 }
            }
            // Recording the reconstructed actions out of scan order.
            2 => {
                plan.functions[0].actions.swap(0, 1);
                LiteralFoldError::DecisionMismatch { function: 0 }
            }
            // Recording one action twice.
            3 => {
                let repeated = plan.functions[0].actions[1];
                plan.functions[0].actions.push(repeated);
                LiteralFoldError::DecisionMismatch { function: 0 }
            }
            // Understating the work two commits cost.
            _ => {
                plan.usage.candidates = 1;
                plan.usage.commits = 1;
                LiteralFoldError::UsageMismatch
            }
        };
        assert_eq!(
            validate(&inputs, &environment, plan).map(|_| ()),
            Err(expected),
            "mutation {mutation}"
        );
    }
}

#[test]
fn a_budget_admitting_one_commit_refuses_the_two_fold_plan() {
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inputs = two_subtract_inputs(target);
    let required = fold_with(&inputs, &environment, LiteralFoldPolicy::EXACT_SUBTRACT_V1)
        .unwrap()
        .plan()
        .usage;
    let generous = budget();
    let one_commit = OptimizationWorkBudget::new(
        required.rule_evaluations,
        1,
        required.validation_steps,
        1,
        required.iterations,
    )
    .unwrap();
    assert!(required.within(generous));
    assert_eq!(
        fold_with_budget(
            &inputs,
            &environment,
            LiteralFoldPolicy::EXACT_SUBTRACT_V1,
            one_commit
        )
        .map(|_| ()),
        Err(LiteralFoldError::BudgetExceeded {
            required,
            budget: one_commit,
        })
    );
}

/// Both folds commit in the one iteration: the published program, with
/// every supporting analysis re-derived over it, offers no further
/// candidate, and repeated computation is deterministic.
#[test]
fn the_two_fold_output_is_a_fixed_point() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let inputs = two_subtract_inputs(target);
        assert_deterministic_fixed_point(
            &inputs,
            &environment,
            LiteralFoldPolicy::EXACT_SUBTRACT_V1,
        );
    }
}
