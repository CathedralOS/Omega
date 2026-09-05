//! Final target-neutral physical homes for every recursive reload segment.

use crate::tests::*;
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use selected_instructions::VirtualRegisterId;

use super::generalized_reload_value_homes::Sources;

pub(super) struct Bundle {
    pub(super) sources: Sources,
    pub(super) prior: selected_instructions_to_register_homes::ValidatedGeneralizedReloadValueHomes,
    pub(super) actions:
        selected_instructions_to_register_homes::ValidatedGeneralizedSpillRecoveryActions,
    pub(super) recursive: selected_instructions_to_register_homes::ValidatedRecursiveSpillInsertion,
}

pub(super) fn reload_bundle(target: NativeTarget) -> Bundle {
    let (sources, actions) = super::recursive_spill_insertion::sources(target);
    let prior = sources.assign(selected_lowering_budget()).unwrap();
    let recursive = sources
        .schedule_recursive_spills(&actions, selected_lowering_budget())
        .unwrap();
    Bundle {
        sources,
        prior,
        actions,
        recursive,
    }
}

pub(super) fn original_bundle(target: NativeTarget) -> Bundle {
    let sources = Sources::from_legality(
        staged_active_resident_original_victim_chain_two_view_legality(target),
    );
    let prior = sources.assign(selected_lowering_budget()).unwrap();
    let worklist = selected_instructions_to_register_homes::seed_generalized_spill_recovery_worklist(
        &prior,
        selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let choices = sources.choose_generalized_victim_with_policy(
        &prior,
        &worklist,
        selected_instructions_to_register_homes::GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1,
        selected_lowering_budget(),
    ).unwrap();
    let actions = sources
        .plan_original_recovery_actions(&prior, &choices, selected_lowering_budget())
        .unwrap();
    let recursive = sources
        .schedule_original_recursive_spills(&actions, selected_lowering_budget())
        .unwrap();
    Bundle {
        sources,
        prior,
        actions,
        recursive,
    }
}

#[test]
fn both_recursive_victim_paths_close_every_reload_segment_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for (original, bundle) in [
            (false, reload_bundle(target)),
            (true, original_bundle(target)),
        ] {
            let first = bundle
                .sources
                .assign_recursive_reload_homes(
                    &bundle.recursive,
                    &bundle.actions,
                    &bundle.prior,
                    selected_lowering_budget(),
                )
                .unwrap();
            let second = bundle
                .sources
                .assign_recursive_reload_homes(
                    &bundle.recursive,
                    &bundle.actions,
                    &bundle.prior,
                    selected_lowering_budget(),
                )
                .unwrap();
            assert_eq!(first, second);
            assert_eq!(first.receipt().assignment_count(), 3);
            assert_eq!(first.receipt().usage(), exact_usage(original));
            assert_eq!(
                first.receipt().retained_home_count(),
                if original { 6 } else { 3 }
            );
            let rows = &first.plan().functions[0].assignments;
            assert_eq!(
                rows.iter().map(|row| row.result).collect::<Vec<_>>(),
                vec![id(0, 0), id(1, 0), id(2, 0)]
            );
            assert_eq!(
                (rows[1].start, rows[1].exclusive_end),
                (LiveRangePoint(14), LiveRangePoint(15))
            );
            assert_eq!(
                (rows[2].start, rows[2].exclusive_end),
                (LiveRangePoint(16), LiveRangePoint(17))
            );
            assert_eq!(
                rows[0].exclusive_end,
                if original {
                    LiveRangePoint(19)
                } else {
                    LiveRangePoint(14)
                }
            );
            let candidates = rows[0].candidates.clone();
            assert_eq!(candidates.len(), 2);
            assert!(rows.iter().all(|row| row.candidates == candidates));
            let low = candidates[0];
            let high = candidates[1];
            assert!(rows.iter().all(|row| row.candidates.contains(&row.view)));
            assert_eq!(
                rows[1].view,
                bundle.actions.plan().actions[0].reclaimed_view
            );
            if original {
                assert!(matches!(
                    rows[2].source,
                    selected_instructions_to_register_homes::RecursiveSpillActionSource::EpochTwoOriginal {
                        victim: VirtualRegisterId(5),
                        ..
                    }
                ));
                assert_eq!(
                    rows.iter().map(|row| row.view).collect::<Vec<_>>(),
                    vec![low, high, high]
                );
                assert_eq!(
                    rows[0].coexisting_homes,
                    vec![
                        home(
                            selected_instructions_to_register_homes::RecursiveReloadCoexistingValue::Original(
                                VirtualRegisterId(4)
                            ),
                            high
                        ),
                        home(
                            selected_instructions_to_register_homes::RecursiveReloadCoexistingValue::Original(
                                VirtualRegisterId(5)
                            ),
                            high
                        ),
                        home(
                            selected_instructions_to_register_homes::RecursiveReloadCoexistingValue::Reload(id(1, 0)),
                            high
                        ),
                        home(
                            selected_instructions_to_register_homes::RecursiveReloadCoexistingValue::Reload(id(2, 0)),
                            high
                        ),
                    ]
                );
                assert_eq!(
                    rows[1].coexisting_homes,
                    vec![home(
                        selected_instructions_to_register_homes::RecursiveReloadCoexistingValue::Reload(id(0, 0)),
                        low,
                    )]
                );
                assert_eq!(
                    rows[2].coexisting_homes,
                    vec![home(
                        selected_instructions_to_register_homes::RecursiveReloadCoexistingValue::Reload(id(0, 0)),
                        low,
                    )]
                );
            } else {
                assert!(
                    matches!(rows[2].source, selected_instructions_to_register_homes::RecursiveSpillActionSource::EpochTwo { victim, .. } if victim == id(0, 0))
                );
                assert!(rows.iter().all(|row| row.view == low));
                assert_eq!(
                    rows[0].coexisting_homes,
                    vec![
                        home(
                            selected_instructions_to_register_homes::RecursiveReloadCoexistingValue::Original(
                                VirtualRegisterId(4)
                            ),
                            high
                        ),
                        home(
                            selected_instructions_to_register_homes::RecursiveReloadCoexistingValue::Original(
                                VirtualRegisterId(5)
                            ),
                            high
                        ),
                    ]
                );
                assert_eq!(
                    rows[1].coexisting_homes,
                    vec![home(
                        selected_instructions_to_register_homes::RecursiveReloadCoexistingValue::Original(
                            VirtualRegisterId(5)
                        ),
                        high,
                    )]
                );
                assert!(rows[2].coexisting_homes.is_empty());
            }
        }
    }
}

#[test]
fn independent_replay_rejects_roots_lineage_interval_domain_view_roster_order_and_usage_corruption()
{
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for bundle in [reload_bundle(target), original_bundle(target)] {
            let canonical = bundle
                .sources
                .assign_recursive_reload_homes(
                    &bundle.recursive,
                    &bundle.actions,
                    &bundle.prior,
                    selected_lowering_budget(),
                )
                .unwrap()
                .plan()
                .clone();

            for corrupt in [
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.recursive_spill_insertion =
                        selected_instructions_to_register_homes::RecursiveSpillInsertionIdentity::from_bytes([0xb0; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.recovery_actions =
                        selected_instructions_to_register_homes::GeneralizedSpillRecoveryActionIdentity::from_bytes(
                            [0xb1; 32],
                        );
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.prior_reload_value_homes =
                        selected_instructions_to_register_homes::GeneralizedReloadValueHomeIdentity::from_bytes([0xb2; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.selected =
                        selected_instructions::SelectedInstructionPlanIdentity::from_bytes(
                            [0xb3; 32],
                        );
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.ranges = selected_instructions_to_register_homes::LiveRangeIdentity::from_bytes([0xb4; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.legality =
                        selected_instructions_to_register_homes::AllocationLegalityIdentity::from_bytes([0xb5; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.register_environment =
                        register_model::TargetRegisterEnvironmentIdentity::from_bytes(
                            [0xb6; 32],
                        );
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.allocator_availability =
                        selected_instructions_to_register_homes::AllocatorAvailabilityIdentity::from_bytes([0xb7; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.optimization_unit =
                        optimization_core::OptimizationUnitIdentity::from_bytes([0xb8; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(99_980).unwrap();
                },
            ] {
                let mut changed = canonical.clone();
                corrupt(&mut changed);
                assert_eq!(
                    bundle.sources.validate_recursive_reload_homes(
                        &bundle.recursive,
                        &bundle.actions,
                        &bundle.prior,
                        changed,
                    ),
                    Err(selected_instructions_to_register_homes::RecursiveReloadValueHomeError::RootMismatch),
                );
            }

            for corrupt in [
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.functions[0].assignments[2].source =
                        selected_instructions_to_register_homes::RecursiveSpillActionSource::EpochTwoOriginal {
                            work_item: selected_instructions_to_register_homes::GeneralizedSpillRecoveryWorkItemId {
                                epoch: 2,
                                ordinal: 0,
                            },
                            source_pressure: id(1, 0),
                            victim: VirtualRegisterId(99),
                        };
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.functions[0].assignments[2].exclusive_end.0 += 1;
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.functions[0].assignments[1].candidates.reverse();
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.functions[0].assignments[1].view =
                        register_model::RegisterViewId(999);
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.functions[0].assignments[0].coexisting_homes.clear();
                },
                |plan: &mut selected_instructions_to_register_homes::RecursiveReloadValueHomePlan| {
                    plan.functions[0].assignments.swap(0, 1);
                },
            ] {
                let mut changed = canonical.clone();
                corrupt(&mut changed);
                assert_eq!(
                    bundle.sources.validate_recursive_reload_homes(
                        &bundle.recursive,
                        &bundle.actions,
                        &bundle.prior,
                        changed,
                    ),
                    Err(
                        selected_instructions_to_register_homes::RecursiveReloadValueHomeError::NonCanonicalAssignments {
                            function: 0
                        }
                    ),
                );
            }

            let mut usage = canonical;
            usage.usage.validation_steps += 1;
            assert_eq!(
                bundle.sources.validate_recursive_reload_homes(
                    &bundle.recursive,
                    &bundle.actions,
                    &bundle.prior,
                    usage,
                ),
                Err(selected_instructions_to_register_homes::RecursiveReloadValueHomeError::UsageMismatch),
            );
        }
    }
}

#[test]
fn exact_envelopes_every_first_over_axis_and_cross_target_roots_fail_closed() {
    for (original, constructor) in [
        (false, reload_bundle as fn(NativeTarget) -> Bundle),
        (true, original_bundle as fn(NativeTarget) -> Bundle),
    ] {
        let usage = exact_usage(original);
        let exact = budget(usage);
        let insufficient = [
            OptimizationWorkBudget::new(
                usage.rule_evaluations - 1,
                usage.candidates,
                usage.validation_steps,
                usage.commits,
                usage.iterations,
            )
            .unwrap(),
            OptimizationWorkBudget::new(
                usage.rule_evaluations,
                usage.candidates - 1,
                usage.validation_steps,
                usage.commits,
                usage.iterations,
            )
            .unwrap(),
            OptimizationWorkBudget::new(
                usage.rule_evaluations,
                usage.candidates,
                usage.validation_steps - 1,
                usage.commits,
                usage.iterations,
            )
            .unwrap(),
            OptimizationWorkBudget::new(
                usage.rule_evaluations,
                usage.candidates,
                usage.validation_steps,
                usage.commits - 1,
                usage.iterations,
            )
            .unwrap(),
            OptimizationWorkBudget::new(
                usage.rule_evaluations,
                usage.candidates,
                usage.validation_steps,
                usage.commits,
                usage.iterations - 1,
            )
            .unwrap(),
        ];
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let bundle = constructor(target);
            assert!(
                bundle
                    .sources
                    .assign_recursive_reload_homes(
                        &bundle.recursive,
                        &bundle.actions,
                        &bundle.prior,
                        exact,
                    )
                    .is_ok()
            );
            for actual in insufficient {
                assert_eq!(
                    bundle.sources.assign_recursive_reload_homes(
                        &bundle.recursive,
                        &bundle.actions,
                        &bundle.prior,
                        actual,
                    ),
                    Err(
                        selected_instructions_to_register_homes::RecursiveReloadValueHomeError::BudgetExceeded {
                            required: usage,
                            budget: actual,
                        }
                    ),
                );
            }
        }

        let x86 = constructor(NativeTarget::linux_x64());
        let foreign = x86
            .sources
            .assign_recursive_reload_homes(&x86.recursive, &x86.actions, &x86.prior, exact)
            .unwrap()
            .plan()
            .clone();
        let arm = constructor(NativeTarget::linux_arm64());
        assert_eq!(
            arm.sources.validate_recursive_reload_homes(
                &arm.recursive,
                &arm.actions,
                &arm.prior,
                foreign,
            ),
            Err(selected_instructions_to_register_homes::RecursiveReloadValueHomeError::RootMismatch),
        );
    }
}

const fn id(
    epoch: u32,
    ordinal: u32,
) -> selected_instructions_to_register_homes::GeneralizedSpillActionId {
    selected_instructions_to_register_homes::GeneralizedSpillActionId { epoch, ordinal }
}

const fn exact_usage(original: bool) -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: 4,
        candidates: 6,
        validation_steps: if original { 30 } else { 27 },
        commits: 3,
        iterations: 4,
    }
}

fn budget(usage: OptimizationWorkUsage) -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(
        usage.rule_evaluations,
        usage.candidates,
        usage.validation_steps,
        usage.commits,
        usage.iterations,
    )
    .unwrap()
}

fn home(
    value: selected_instructions_to_register_homes::RecursiveReloadCoexistingValue,
    view: register_model::RegisterViewId,
) -> selected_instructions_to_register_homes::RecursiveReloadCoexistingHome {
    selected_instructions_to_register_homes::RecursiveReloadCoexistingHome {
        value,
        class: register_model::RegisterClassId(0),
        view,
    }
}
