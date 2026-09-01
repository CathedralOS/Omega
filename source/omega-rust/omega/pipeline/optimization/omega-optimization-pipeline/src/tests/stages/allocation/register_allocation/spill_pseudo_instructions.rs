//! Compiler-private spill pseudos projected from the recursive logical schedule.

use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;

use super::{
    generalized_reload_value_homes::Sources,
    recursive_spill_insertion::sources as recursive_sources,
};

fn sources(target: NativeTarget) -> (Sources, omega_regalloc::ValidatedRecursiveSpillInsertion) {
    let (sources, actions) = recursive_sources(target);
    let recursive = sources
        .schedule_recursive_spills(&actions, selected_lowering_budget())
        .unwrap();
    (sources, recursive)
}

fn lower(
    source: &omega_regalloc::ValidatedRecursiveSpillInsertion,
    budget: OptimizationWorkBudget,
) -> Result<
    omega_regalloc::ValidatedSpillPseudoInstructions,
    omega_regalloc::SpillPseudoInstructionError,
> {
    omega_regalloc::lower_recursive_spill_pseudos(
        source,
        omega_regalloc::SpillPseudoInstructionPolicy::RecursiveLogicalScheduleV1,
        budget,
    )
}

#[test]
fn recursive_schedule_becomes_linked_target_neutral_pseudos_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (_, recursive) = sources(target);
        let first = lower(&recursive, selected_lowering_budget()).unwrap();
        let second = lower(&recursive, selected_lowering_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().storage_count(), 3);
        assert_eq!(first.receipt().instruction_count(), 6);
        assert_eq!(first.receipt().rewrite_count(), 4);
        assert_eq!(first.receipt().max_spill_area_bytes(), 16);
        assert_eq!(first.receipt().usage(), exact_usage());
        assert_eq!(
            first.receipt().recursive_spill_insertion(),
            recursive.receipt().identity()
        );

        let function = &first.plan().functions[0];
        assert_eq!(function.spill_area_bytes, 16);
        assert_eq!(
            function
                .instructions
                .iter()
                .copied()
                .map(omega_regalloc::SpillPseudoInstruction::id)
                .map(|id| id.ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5]
        );
        assert!(matches!(
            function.instructions[1],
            omega_regalloc::SpillPseudoInstruction::Store {
                action,
                before_reload: Some(omega_regalloc::SpillPseudoInstructionId { ordinal: 2 }),
                source: omega_regalloc::SpillPseudoStoredValue::Original(_),
                ..
            } if action == id(1, 0)
        ));
        assert!(matches!(
            function.instructions[3],
            omega_regalloc::SpillPseudoInstruction::Store {
                action,
                before_reload: Some(omega_regalloc::SpillPseudoInstructionId { ordinal: 4 }),
                source: omega_regalloc::SpillPseudoStoredValue::Reload {
                    action: source,
                    producer: omega_regalloc::SpillPseudoInstructionId { ordinal: 2 },
                },
                ..
            } if action == id(2, 0) && source == id(0, 0)
        ));
        assert!(function.rewrites.iter().any(|rewrite| {
            rewrite.action == id(2, 0)
                && rewrite.result == id(2, 0)
                && rewrite.point == LiveRangePoint(16)
                && rewrite.producer.ordinal == 5
        }));
    }
}

#[test]
fn independent_replay_rejects_every_root_and_pseudo_surface_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (_, recursive) = sources(target);
        let canonical = lower(&recursive, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();
        for corrupt in [
            |plan: &mut omega_regalloc::SpillPseudoInstructionPlan| {
                plan.recursive_spill_insertion =
                    omega_regalloc::RecursiveSpillInsertionIdentity::from_bytes([0xf1; 32]);
            },
            |plan: &mut omega_regalloc::SpillPseudoInstructionPlan| {
                plan.register_environment =
                    omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xf2; 32]);
            },
            |plan: &mut omega_regalloc::SpillPseudoInstructionPlan| {
                plan.allocator_availability =
                    omega_regalloc::AllocatorAvailabilityIdentity::from_bytes([0xf3; 32]);
            },
            |plan: &mut omega_regalloc::SpillPseudoInstructionPlan| {
                plan.optimization_unit =
                    omega_optimization_core::OptimizationUnitIdentity::from_bytes([0xf4; 32]);
            },
            |plan: &mut omega_regalloc::SpillPseudoInstructionPlan| {
                plan.fuel_schedule = psi_core::FuelScheduleIdentity::new(99_960).unwrap();
            },
        ] {
            let mut changed = canonical.clone();
            corrupt(&mut changed);
            assert_eq!(
                omega_regalloc::validate_spill_pseudo_instructions(&recursive, changed),
                Err(omega_regalloc::SpillPseudoInstructionError::RootMismatch)
            );
        }

        let mutations: [fn(&mut omega_regalloc::SpillPseudoInstructionPlan); 6] = [
            |plan| plan.functions[0].storage[2].spill_area_offset += 8,
            |plan| match &mut plan.functions[0].instructions[3] {
                omega_regalloc::SpillPseudoInstruction::Store { id, .. } => id.ordinal += 1,
                _ => unreachable!(),
            },
            |plan| match &mut plan.functions[0].instructions[3] {
                omega_regalloc::SpillPseudoInstruction::Store { before_reload, .. } => {
                    *before_reload = None
                }
                _ => unreachable!(),
            },
            |plan| match &mut plan.functions[0].instructions[3] {
                omega_regalloc::SpillPseudoInstruction::Store { source, .. } => {
                    *source = omega_regalloc::SpillPseudoStoredValue::Original(
                        omega_selected_instructions::VirtualRegisterId(0),
                    )
                }
                _ => unreachable!(),
            },
            |plan| plan.functions[0].rewrites[3].producer.ordinal -= 1,
            |plan| {
                plan.functions[0].instructions.pop();
            },
        ];
        for mutate in mutations {
            let mut changed = canonical.clone();
            mutate(&mut changed);
            assert_eq!(
                omega_regalloc::validate_spill_pseudo_instructions(&recursive, changed),
                Err(omega_regalloc::SpillPseudoInstructionError::NonCanonicalFunctions)
            );
        }

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            omega_regalloc::validate_spill_pseudo_instructions(&recursive, usage),
            Err(omega_regalloc::SpillPseudoInstructionError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_each_representable_axis_and_cross_target_roots_fail_closed() {
    let exact = OptimizationWorkBudget::new(1, 6, 23, 10, 14).unwrap();
    let insufficient = [
        OptimizationWorkBudget::new(1, 5, 23, 10, 14).unwrap(),
        OptimizationWorkBudget::new(1, 6, 22, 10, 14).unwrap(),
        OptimizationWorkBudget::new(1, 6, 23, 9, 14).unwrap(),
        OptimizationWorkBudget::new(1, 6, 23, 10, 13).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (_, recursive) = sources(target);
        assert!(lower(&recursive, exact).is_ok());
        for budget in insufficient {
            assert!(matches!(
                lower(&recursive, budget),
                Err(omega_regalloc::SpillPseudoInstructionError::BudgetExceeded {
                    required,
                    budget: actual,
                }) if required == exact_usage() && actual == budget
            ));
        }
    }

    let (_, x86) = sources(NativeTarget::linux_x64());
    let foreign = lower(&x86, exact).unwrap().plan().clone();
    let (_, arm) = sources(NativeTarget::linux_arm64());
    assert_eq!(
        omega_regalloc::validate_spill_pseudo_instructions(&arm, foreign),
        Err(omega_regalloc::SpillPseudoInstructionError::RootMismatch)
    );
}

const fn id(epoch: u32, ordinal: u32) -> omega_regalloc::GeneralizedSpillActionId {
    omega_regalloc::GeneralizedSpillActionId { epoch, ordinal }
}

const fn exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 6,
        validation_steps: 23,
        commits: 10,
        iterations: 14,
    }
}
