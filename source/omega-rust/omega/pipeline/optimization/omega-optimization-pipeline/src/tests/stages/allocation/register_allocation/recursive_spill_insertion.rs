//! Complete epoch-zero/two schedule after recursive logical insertion and recoloring.

use crate::tests::*;
use omega_optimization_core::OptimizationWorkUsage;

use super::generalized_reload_value_homes::Sources;

pub(super) fn sources(
    target: NativeTarget,
) -> (
    Sources,
    omega_regalloc::ValidatedGeneralizedSpillRecoveryActions,
) {
    let sources = Sources::new(target);
    let homes = sources.assign(selected_lowering_budget()).unwrap();
    let worklist = omega_regalloc::seed_generalized_spill_recovery_worklist(
        &homes,
        omega_regalloc::GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let choices = sources
        .choose_generalized_victim(&homes, &worklist, selected_lowering_budget())
        .unwrap();
    let actions = sources
        .plan_generalized_recovery_actions(&homes, &choices, selected_lowering_budget())
        .unwrap();
    (sources, actions)
}

#[test]
fn epoch_two_extends_one_schedule_and_reuses_the_disjoint_epoch_zero_offset() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, actions) = sources(target);
        let first = sources
            .schedule_recursive_spills(&actions, selected_lowering_budget())
            .unwrap();
        let second = sources
            .schedule_recursive_spills(&actions, selected_lowering_budget())
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().action_count(), 3);
        assert_eq!(first.receipt().event_count(), 10);
        assert_eq!(first.receipt().max_spill_area_bytes(), 16);
        assert_eq!(first.receipt().usage(), exact_usage());

        let function = &first.plan().functions[0];
        assert_eq!(function.spill_area_bytes, 16);
        assert_eq!(function.slots.len(), 3);
        assert_eq!(function.slots[0].action, id(0, 0));
        assert_eq!(function.slots[0].spill_area_offset, 0);
        assert_eq!(function.slots[1].action, id(1, 0));
        assert_eq!(function.slots[1].spill_area_offset, 8);
        assert_eq!(function.slots[2].action, id(2, 0));
        assert_eq!(function.slots[2].live_from, LiveRangePoint(14));
        assert_eq!(function.slots[2].live_through, LiveRangePoint(16));
        assert_eq!(function.slots[2].spill_area_offset, 0);

        let epoch_two_store = function
            .schedule
            .iter()
            .find(|event| {
                matches!(
                    event,
                    omega_regalloc::RecursiveSpillEvent::Store { action, .. } if *action == id(2, 0)
                )
            })
            .unwrap();
        let omega_regalloc::RecursiveSpillEvent::Store {
            point,
            before_instruction,
            before_reload,
            source,
            ..
        } = *epoch_two_store
        else {
            unreachable!()
        };
        assert_eq!(point, LiveRangePoint(14));
        assert_eq!(before_instruction.0, 7);
        assert_eq!(before_reload, Some(id(1, 0)));
        assert_eq!(
            source,
            omega_regalloc::RecursiveSpillStoredValue::Reload(id(0, 0))
        );
        assert!(matches!(
            function.schedule.iter().find(|event| matches!(
                event,
                omega_regalloc::RecursiveSpillEvent::Reload { action, .. } if *action == id(2, 0)
            )),
            Some(omega_regalloc::RecursiveSpillEvent::Reload { point: LiveRangePoint(16), before_instruction, .. }) if before_instruction.0 == 8
        ));
    }
}

#[test]
fn reload_victim_v1_recursive_identity_remains_byte_stable() {
    let (sources, actions) = sources(NativeTarget::linux_x64());
    let scheduled = sources
        .schedule_recursive_spills(
            &actions,
            OptimizationWorkBudget::new(1, 3, 14, 3, 4).unwrap(),
        )
        .unwrap();
    assert_eq!(
        scheduled.receipt().identity().bytes(),
        [
            206, 40, 72, 12, 162, 53, 37, 127, 235, 175, 60, 77, 250, 11, 114, 132, 244, 25, 105,
            79, 187, 90, 52, 102, 138, 241, 157, 226, 8, 96, 12, 8,
        ]
    );
    assert_eq!(
        sources.schedule_original_recursive_spills(
            &actions,
            OptimizationWorkBudget::new(1, 3, 14, 3, 4).unwrap(),
        ),
        Err(
            omega_regalloc::RecursiveSpillInsertionError::UnsupportedRecoveryVictim {
                function: 0,
                action: id(2, 0),
                victim: omega_regalloc::GeneralizedSpillRecoveryVictim::Reload(id(0, 0)),
            }
        )
    );
}

#[test]
fn independent_replay_rejects_root_slot_schedule_and_usage_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, actions) = sources(target);
        let canonical = sources
            .schedule_recursive_spills(&actions, selected_lowering_budget())
            .unwrap()
            .plan()
            .clone();

        for corrupt in [
            |plan: &mut omega_regalloc::RecursiveSpillInsertionPlan| {
                plan.generalized_spill_insertion =
                    omega_regalloc::GeneralizedSpillInsertionIdentity::from_bytes([0xe1; 32]);
            },
            |plan: &mut omega_regalloc::RecursiveSpillInsertionPlan| {
                plan.recovery_actions =
                    omega_regalloc::GeneralizedSpillRecoveryActionIdentity::from_bytes([0xe2; 32]);
            },
            |plan: &mut omega_regalloc::RecursiveSpillInsertionPlan| {
                plan.register_environment =
                    omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xe3; 32]);
            },
            |plan: &mut omega_regalloc::RecursiveSpillInsertionPlan| {
                plan.allocator_availability =
                    omega_regalloc::AllocatorAvailabilityIdentity::from_bytes([0xe4; 32]);
            },
            |plan: &mut omega_regalloc::RecursiveSpillInsertionPlan| {
                plan.optimization_unit =
                    omega_optimization_core::OptimizationUnitIdentity::from_bytes([0xe5; 32]);
            },
            |plan: &mut omega_regalloc::RecursiveSpillInsertionPlan| {
                plan.fuel_schedule = psi_core::FuelScheduleIdentity::new(99_950).unwrap();
            },
        ] {
            let mut root = canonical.clone();
            corrupt(&mut root);
            assert_eq!(
                sources.validate_recursive_spills(&actions, root),
                Err(omega_regalloc::RecursiveSpillInsertionError::RootMismatch)
            );
        }

        let mut slot = canonical.clone();
        slot.functions[0].slots[2].spill_area_offset = 8;
        assert_eq!(
            sources.validate_recursive_spills(&actions, slot),
            Err(omega_regalloc::RecursiveSpillInsertionError::NonCanonicalSlots { function: 0 })
        );

        let mut event = canonical.clone();
        let row = event.functions[0]
            .schedule
            .iter_mut()
            .find(|event| {
                matches!(
                    event,
                    omega_regalloc::RecursiveSpillEvent::Store { action, .. } if *action == id(2, 0)
                )
            })
            .unwrap();
        let omega_regalloc::RecursiveSpillEvent::Store { source, .. } = row else {
            unreachable!()
        };
        *source = omega_regalloc::RecursiveSpillStoredValue::Original(
            omega_selected_instructions::VirtualRegisterId(0),
        );
        assert_eq!(
            sources.validate_recursive_spills(&actions, event),
            Err(omega_regalloc::RecursiveSpillInsertionError::NonCanonicalSchedule { function: 0 })
        );

        let mut usage = canonical;
        usage.usage.iterations += 1;
        assert_eq!(
            sources.validate_recursive_spills(&actions, usage),
            Err(omega_regalloc::RecursiveSpillInsertionError::UsageMismatch)
        );
    }
}

#[test]
fn exact_budget_and_cross_target_roots_fail_closed() {
    let exact = OptimizationWorkBudget::new(1, 3, 14, 3, 4).unwrap();
    let insufficient = [
        OptimizationWorkBudget::new(1, 2, 14, 3, 4).unwrap(),
        OptimizationWorkBudget::new(1, 3, 13, 3, 4).unwrap(),
        OptimizationWorkBudget::new(1, 3, 14, 2, 4).unwrap(),
        OptimizationWorkBudget::new(1, 3, 14, 3, 3).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, actions) = sources(target);
        assert!(sources.schedule_recursive_spills(&actions, exact).is_ok());
        for budget in insufficient {
            assert!(matches!(
                sources.schedule_recursive_spills(&actions, budget),
                Err(omega_regalloc::RecursiveSpillInsertionError::BudgetExceeded { required, budget: actual })
                    if required == exact_usage() && actual == budget
            ));
        }
    }

    let (x86, x86_actions) = sources(NativeTarget::linux_x64());
    let foreign = x86
        .schedule_recursive_spills(&x86_actions, exact)
        .unwrap()
        .plan()
        .clone();
    let (arm, arm_actions) = sources(NativeTarget::linux_arm64());
    assert_eq!(
        arm.validate_recursive_spills(&arm_actions, foreign),
        Err(omega_regalloc::RecursiveSpillInsertionError::RootMismatch)
    );
}

const fn id(epoch: u32, ordinal: u32) -> omega_regalloc::GeneralizedSpillActionId {
    omega_regalloc::GeneralizedSpillActionId { epoch, ordinal }
}

const fn exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 3,
        validation_steps: 14,
        commits: 3,
        iterations: 4,
    }
}
