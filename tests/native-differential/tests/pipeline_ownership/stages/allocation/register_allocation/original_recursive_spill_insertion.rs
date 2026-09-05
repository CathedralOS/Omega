//! Complete target-neutral schedule for the guarded epoch-two original victim.

use crate::tests::*;
use optimization_core::OptimizationWorkUsage;
use selected_instructions::VirtualRegisterId;

use super::generalized_reload_value_homes::Sources;

fn sources(
    target: NativeTarget,
) -> (
    Sources,
    selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
) {
    let sources = Sources::from_legality(
        staged_active_resident_original_victim_chain_two_view_legality(target),
    );
    let homes = sources.assign(selected_lowering_budget()).unwrap();
    let worklist = selected_instructions_to_register_homes::seed_generalized_spill_recovery_worklist(
        &homes,
        selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let choices = sources
        .choose_generalized_victim_with_policy(
            &homes,
            &worklist,
            selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1,
            selected_lowering_budget(),
        )
        .unwrap();
    let actions = sources
        .plan_original_recovery_actions(&homes, &choices, selected_lowering_budget())
        .unwrap();
    (sources, actions)
}

fn lower_pseudos(
    source: &selected_instructions_to_register_homes::ValidatedRecursiveSpillInsertion,
) -> selected_instructions_to_register_homes::ValidatedSpillPseudoInstructions {
    selected_instructions_to_register_homes::lower_recursive_spill_pseudos(
        source,
        selected_instructions_to_register_homes::SpillPseudoInstructionPolicy::RecursiveLogicalScheduleV1,
        selected_lowering_budget(),
    )
    .unwrap()
}

#[test]
fn original_victim_extends_the_recursive_schedule_and_crosses_spill_pseudos() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, actions) = sources(target);
        let first = sources
            .schedule_original_recursive_spills(&actions, exact_budget())
            .unwrap();
        let second = sources
            .schedule_original_recursive_spills(&actions, exact_budget())
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().action_count(), 3);
        assert_eq!(first.receipt().event_count(), 11);
        assert_eq!(first.receipt().max_spill_area_bytes(), 16);
        assert_eq!(first.receipt().usage(), exact_usage());

        let function = &first.plan().functions[0];
        assert_eq!(function.spill_area_bytes, 16);
        assert_eq!(function.slots.len(), 3);
        assert_eq!(function.slots[0].spill_area_offset, 0);
        assert_eq!(function.slots[1].spill_area_offset, 8);
        assert_eq!(function.slots[2].action, id(2, 0));
        assert_eq!(function.slots[2].live_from, LiveRangePoint(14));
        assert_eq!(function.slots[2].live_through, LiveRangePoint(16));
        assert_eq!(function.slots[2].spill_area_offset, 0);
        assert_eq!(
            function.slots[2].source,
            selected_instructions_to_register_homes::RecursiveSpillActionSource::EpochTwoOriginal {
                work_item:
                    selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorkItemId {
                        epoch: 2,
                        ordinal: 0,
                    },
                source_pressure: id(1, 0),
                victim: VirtualRegisterId(5),
            }
        );

        let store = function
            .schedule
            .iter()
            .find(|event| matches!(event, selected_instructions_to_register_homes::RecursiveSpillEvent::Store { action, .. } if *action == id(2, 0)))
            .unwrap();
        assert!(matches!(
            store,
            selected_instructions_to_register_homes::RecursiveSpillEvent::Store {
                point: LiveRangePoint(14),
                before_instruction,
                before_reload: Some(source_pressure),
                source: selected_instructions_to_register_homes::RecursiveSpillStoredValue::Original(VirtualRegisterId(5)),
                ..
            } if before_instruction.0 == 7 && *source_pressure == id(1, 0)
        ));
        assert!(matches!(
            function.schedule.iter().find(|event| matches!(event, selected_instructions_to_register_homes::RecursiveSpillEvent::Reload { action, .. } if *action == id(2, 0))),
            Some(selected_instructions_to_register_homes::RecursiveSpillEvent::Reload {
                point: LiveRangePoint(16),
                before_instruction,
                ..
            }) if before_instruction.0 == 8
        ));

        let pseudos = lower_pseudos(&first);
        let pseudo_function = &pseudos.plan().functions[0];
        assert!(matches!(
            pseudo_function.instructions[3],
            selected_instructions_to_register_homes::SpillPseudoInstruction::Store {
                action,
                before_reload: Some(selected_instructions_to_register_homes::SpillPseudoInstructionId { ordinal: 4 }),
                source: selected_instructions_to_register_homes::SpillPseudoStoredValue::Original(VirtualRegisterId(5)),
                ..
            } if action == id(2, 0)
        ));
        assert_eq!(pseudos.receipt().max_spill_area_bytes(), 16);
    }
}

#[test]
fn independent_replay_rejects_original_lineage_schedule_policy_and_usage_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, actions) = sources(target);
        let canonical = sources
            .schedule_original_recursive_spills(&actions, exact_budget())
            .unwrap()
            .plan()
            .clone();

        for corrupt in [
            |plan: &mut selected_instructions_to_register_homes::RecursiveSpillInsertionPlan| {
                plan.generalized_spill_insertion =
                    selected_instructions_to_register_homes::GeneralizedSpillInsertionIdentity::from_bytes([0xd0; 32])
            },
            |plan: &mut selected_instructions_to_register_homes::RecursiveSpillInsertionPlan| {
                plan.recovery_actions =
                    selected_instructions_to_register_homes::GeneralizedSpillRecoveryActionIdentity::from_bytes([0xd1; 32])
            },
            |plan: &mut selected_instructions_to_register_homes::RecursiveSpillInsertionPlan| {
                plan.register_environment =
                    register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xd2; 32])
            },
            |plan: &mut selected_instructions_to_register_homes::RecursiveSpillInsertionPlan| {
                plan.allocator_availability =
                    selected_instructions_to_register_homes::AllocatorAvailabilityIdentity::from_bytes([0xd3; 32])
            },
            |plan: &mut selected_instructions_to_register_homes::RecursiveSpillInsertionPlan| {
                plan.optimization_unit =
                    optimization_core::OptimizationUnitIdentity::from_bytes([0xd4; 32])
            },
            |plan: &mut selected_instructions_to_register_homes::RecursiveSpillInsertionPlan| {
                plan.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(99_970).unwrap()
            },
        ] {
            let mut root = canonical.clone();
            corrupt(&mut root);
            assert_eq!(
                sources.validate_recursive_spills(&actions, root),
                Err(selected_instructions_to_register_homes::RecursiveSpillInsertionError::RootMismatch)
            );
        }

        let mut wrong_policy = canonical.clone();
        wrong_policy.policy = selected_instructions_to_register_homes::RecursiveSpillInsertionPolicy::EpochTwoReloadVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV1;
        assert_eq!(
            sources.validate_recursive_spills(&actions, wrong_policy),
            Err(
                selected_instructions_to_register_homes::RecursiveSpillInsertionError::UnsupportedRecoveryVictim {
                    function: 0,
                    action: id(2, 0),
                    victim: selected_instructions_to_register_homes::GeneralizedSpillRecoveryVictim::Original(
                        VirtualRegisterId(5),
                    ),
                }
            )
        );

        let mut lineage = canonical.clone();
        lineage.functions[0].slots[2].source =
            selected_instructions_to_register_homes::RecursiveSpillActionSource::EpochTwoOriginal {
                work_item:
                    selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorkItemId {
                        epoch: 2,
                        ordinal: 0,
                    },
                source_pressure: id(1, 0),
                victim: VirtualRegisterId(6),
            };
        assert_eq!(
            sources.validate_recursive_spills(&actions, lineage),
            Err(selected_instructions_to_register_homes::RecursiveSpillInsertionError::NonCanonicalSlots { function: 0 })
        );

        let mut schedule = canonical.clone();
        let row = schedule.functions[0]
            .schedule
            .iter_mut()
            .find(|event| matches!(event, selected_instructions_to_register_homes::RecursiveSpillEvent::Store { action, .. } if *action == id(2, 0)))
            .unwrap();
        let selected_instructions_to_register_homes::RecursiveSpillEvent::Store { source, .. } =
            row
        else {
            unreachable!()
        };
        *source =
            selected_instructions_to_register_homes::RecursiveSpillStoredValue::Reload(id(0, 0));
        assert_eq!(
            sources.validate_recursive_spills(&actions, schedule),
            Err(selected_instructions_to_register_homes::RecursiveSpillInsertionError::NonCanonicalSchedule { function: 0 })
        );

        let mut usage = canonical;
        usage.usage.validation_steps += 1;
        assert_eq!(
            sources.validate_recursive_spills(&actions, usage),
            Err(selected_instructions_to_register_homes::RecursiveSpillInsertionError::UsageMismatch)
        );
    }
}

#[test]
fn original_recursive_budget_and_cross_target_custody_fail_closed() {
    let insufficient = [
        OptimizationWorkBudget::new(1, 2, 15, 3, 4).unwrap(),
        OptimizationWorkBudget::new(1, 3, 14, 3, 4).unwrap(),
        OptimizationWorkBudget::new(1, 3, 15, 2, 4).unwrap(),
        OptimizationWorkBudget::new(1, 3, 15, 3, 3).unwrap(),
    ];
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (sources, actions) = sources(target);
        assert!(
            sources
                .schedule_original_recursive_spills(&actions, exact_budget())
                .is_ok()
        );
        for budget in insufficient {
            assert!(matches!(
                sources.schedule_original_recursive_spills(&actions, budget),
                Err(selected_instructions_to_register_homes::RecursiveSpillInsertionError::BudgetExceeded {
                    required,
                    budget: actual,
                }) if required == exact_usage() && actual == budget
            ));
        }
    }

    let (x86, x86_actions) = sources(NativeTarget::linux_x64());
    let foreign = x86
        .schedule_original_recursive_spills(&x86_actions, exact_budget())
        .unwrap()
        .plan()
        .clone();
    let (arm, arm_actions) = sources(NativeTarget::linux_arm64());
    assert_eq!(
        arm.validate_recursive_spills(&arm_actions, foreign),
        Err(selected_instructions_to_register_homes::RecursiveSpillInsertionError::RootMismatch)
    );
}

const fn id(
    epoch: u32,
    ordinal: u32,
) -> selected_instructions_to_register_homes::GeneralizedSpillActionId {
    selected_instructions_to_register_homes::GeneralizedSpillActionId { epoch, ordinal }
}

fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1, 3, 15, 3, 4).unwrap()
}

const fn exact_usage() -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 3,
        validation_steps: 15,
        commits: 3,
        iterations: 4,
    }
}
