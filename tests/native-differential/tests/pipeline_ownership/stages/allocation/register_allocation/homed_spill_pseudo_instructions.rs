//! V2 target-neutral spill pseudos with exact reload destination views.

use crate::tests::*;
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use super::recursive_reload_value_homes::{Bundle, original_bundle, reload_bundle};

pub(super) struct HomedBundle {
    pub(super) bundle: Bundle,
    pub(super) pseudos: selected_instructions_to_register_homes::ValidatedSpillPseudoInstructions,
    pub(super) homes: selected_instructions_to_register_homes::ValidatedRecursiveReloadValueHomes,
}

pub(super) fn build(constructor: fn(NativeTarget) -> Bundle, target: NativeTarget) -> HomedBundle {
    let bundle = constructor(target);
    let pseudos = selected_instructions_to_register_homes::lower_recursive_spill_pseudos(
        &bundle.recursive,
        selected_instructions_to_register_homes::SpillPseudoInstructionPolicy::RecursiveLogicalScheduleV1,
        selected_lowering_budget(),
    )
    .unwrap();
    let homes = bundle
        .sources
        .assign_recursive_reload_homes(
            &bundle.recursive,
            &bundle.actions,
            &bundle.prior,
            selected_lowering_budget(),
        )
        .unwrap();
    HomedBundle {
        bundle,
        pseudos,
        homes,
    }
}

pub(super) fn lower(
    source: &HomedBundle,
    budget: OptimizationWorkBudget,
) -> Result<
    selected_instructions_to_register_homes::ValidatedHomedSpillPseudoInstructions,
    selected_instructions_to_register_homes::HomedSpillPseudoInstructionError,
> {
    selected_instructions_to_register_homes::lower_homed_recursive_spill_pseudos(
        &source.pseudos,
        &source.homes,
        selected_instructions_to_register_homes::HomedSpillPseudoInstructionPolicy::RecursiveLogicalScheduleWithClosedReloadHomesV2,
        budget,
    )
}

fn validate(
    source: &HomedBundle,
    plan: selected_instructions_to_register_homes::HomedSpillPseudoInstructionPlan,
) -> Result<
    selected_instructions_to_register_homes::ValidatedHomedSpillPseudoInstructions,
    selected_instructions_to_register_homes::HomedSpillPseudoInstructionError,
> {
    selected_instructions_to_register_homes::validate_homed_spill_pseudo_instructions(
        &source.pseudos,
        &source.homes,
        plan,
    )
}

#[test]
fn both_recursive_paths_gain_exact_destination_views_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for (original, constructor) in [
            (false, reload_bundle as fn(NativeTarget) -> Bundle),
            (true, original_bundle as fn(NativeTarget) -> Bundle),
        ] {
            let source = build(constructor, target);
            let first = lower(&source, selected_lowering_budget()).unwrap();
            let second = lower(&source, selected_lowering_budget()).unwrap();
            assert_eq!(first, second);
            assert_eq!(first.receipt().storage_count(), 3);
            assert_eq!(first.receipt().instruction_count(), 6);
            assert_eq!(first.receipt().reload_count(), 3);
            assert_eq!(
                first.receipt().rewrite_count(),
                if original { 5 } else { 4 }
            );
            assert_eq!(first.receipt().max_spill_area_bytes(), 16);
            assert_eq!(first.receipt().usage(), exact_usage(original));
            assert_eq!(
                first.receipt().spill_pseudo_instructions(),
                source.pseudos.receipt().identity(),
            );
            assert_eq!(
                first.receipt().recursive_reload_value_homes(),
                source.homes.receipt().identity(),
            );

            let function = &first.plan().functions[0];
            assert_eq!(function.storage, source.pseudos.plan().functions[0].storage);
            assert_eq!(
                function.rewrites,
                source.pseudos.plan().functions[0].rewrites
            );
            assert_eq!(
                function
                    .instructions
                    .iter()
                    .copied()
                    .map(selected_instructions_to_register_homes::HomedSpillPseudoInstruction::id)
                    .map(|id| id.ordinal)
                    .collect::<Vec<_>>(),
                vec![0, 1, 2, 3, 4, 5],
            );
            let reloads = function
                .instructions
                .iter()
                .filter_map(|instruction| {
                    match instruction {
                    selected_instructions_to_register_homes::HomedSpillPseudoInstruction::Reload {
                        result,
                        destination_view,
                        ..
                    } => Some((*result, *destination_view)),
                    _ => None,
                }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                reloads,
                source.homes.plan().functions[0]
                    .assignments
                    .iter()
                    .map(|home| (home.result, home.view))
                    .collect::<Vec<_>>(),
            );
            assert!(
                matches!(
                    function.instructions[3],
                    selected_instructions_to_register_homes::HomedSpillPseudoInstruction::Store {
                        action,
                        before_reload: Some(selected_instructions_to_register_homes::SpillPseudoInstructionId { ordinal: 4 }),
                        source: selected_instructions_to_register_homes::SpillPseudoStoredValue::Reload { action: source, producer: selected_instructions_to_register_homes::SpillPseudoInstructionId { ordinal: 2 } },
                        ..
                    } if !original && action == id(2, 0) && source == id(0, 0)
                ) || matches!(
                    function.instructions[3],
                    selected_instructions_to_register_homes::HomedSpillPseudoInstruction::Store {
                        action,
                        before_reload: Some(selected_instructions_to_register_homes::SpillPseudoInstructionId { ordinal: 4 }),
                        source: selected_instructions_to_register_homes::SpillPseudoStoredValue::Original(
                            selected_instructions::VirtualRegisterId(5)
                        ),
                        ..
                    } if original && action == id(2, 0)
                )
            );
        }
    }
}

#[test]
fn legacy_v1_identity_and_signature_remain_byte_stable() {
    let source = build(reload_bundle, NativeTarget::linux_x64());
    let v1 = selected_instructions_to_register_homes::lower_recursive_spill_pseudos(
        &source.bundle.recursive,
        selected_instructions_to_register_homes::SpillPseudoInstructionPolicy::RecursiveLogicalScheduleV1,
        OptimizationWorkBudget::new(1, 6, 23, 10, 14).unwrap(),
    )
    .unwrap();
    // Full-pipeline golden includes the upstream proof vocabulary version 24.
    assert_eq!(
        v1.receipt().identity().bytes(),
        [
            183, 152, 68, 176, 254, 110, 32, 170, 55, 38, 19, 193, 242, 76, 152, 172, 168, 171,
            161, 0, 82, 63, 255, 253, 76, 135, 190, 181, 89, 29, 215, 38
        ],
    );
}

#[test]
fn replay_rejects_every_root_destination_view_and_v1_surface_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for constructor in [
            reload_bundle as fn(NativeTarget) -> Bundle,
            original_bundle as fn(NativeTarget) -> Bundle,
        ] {
            let source = build(constructor, target);
            let canonical = lower(&source, selected_lowering_budget())
                .unwrap()
                .plan()
                .clone();
            for corrupt in [
                |plan: &mut selected_instructions_to_register_homes::HomedSpillPseudoInstructionPlan| {
                    plan.spill_pseudo_instructions =
                        selected_instructions_to_register_homes::SpillPseudoInstructionPlanIdentity::from_bytes([0xc0; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::HomedSpillPseudoInstructionPlan| {
                    plan.recursive_reload_value_homes =
                        selected_instructions_to_register_homes::RecursiveReloadValueHomeIdentity::from_bytes([0xc1; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::HomedSpillPseudoInstructionPlan| {
                    plan.register_environment =
                        register_model::TargetRegisterEnvironmentIdentity::from_bytes(
                            [0xc2; 32],
                        );
                },
                |plan: &mut selected_instructions_to_register_homes::HomedSpillPseudoInstructionPlan| {
                    plan.allocator_availability =
                        selected_instructions_to_register_homes::AllocatorAvailabilityIdentity::from_bytes([0xc3; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::HomedSpillPseudoInstructionPlan| {
                    plan.optimization_unit =
                        optimization_core::OptimizationUnitIdentity::from_bytes([0xc4; 32]);
                },
                |plan: &mut selected_instructions_to_register_homes::HomedSpillPseudoInstructionPlan| {
                    plan.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(99_990).unwrap();
                },
            ] {
                let mut changed = canonical.clone();
                corrupt(&mut changed);
                assert_eq!(
                    validate(&source, changed),
                    Err(selected_instructions_to_register_homes::HomedSpillPseudoInstructionError::RootMismatch),
                );
            }

            let mutations: [fn(
                &mut selected_instructions_to_register_homes::HomedSpillPseudoInstructionPlan,
            ); 6] = [
                |plan| plan.functions[0].storage[2].spill_area_offset += 8,
                |plan| {
                    match &mut plan.functions[0].instructions[0] {
                    selected_instructions_to_register_homes::HomedSpillPseudoInstruction::Store { source_view, .. } => {
                        source_view.0 += 1
                    }
                    _ => unreachable!(),
                }
                },
                |plan| {
                    match &mut plan.functions[0].instructions[2] {
                    selected_instructions_to_register_homes::HomedSpillPseudoInstruction::Reload {
                        destination_class,
                        ..
                    } => destination_class.0 += 1,
                    _ => unreachable!(),
                }
                },
                |plan| {
                    match &mut plan.functions[0].instructions[2] {
                    selected_instructions_to_register_homes::HomedSpillPseudoInstruction::Reload {
                        destination_view,
                        ..
                    } => destination_view.0 += 1,
                    _ => unreachable!(),
                }
                },
                |plan| plan.functions[0].rewrites[0].producer.ordinal += 1,
                |plan| {
                    plan.functions[0].instructions.swap(0, 1);
                },
            ];
            for mutate in mutations {
                let mut changed = canonical.clone();
                mutate(&mut changed);
                assert_eq!(
                    validate(&source, changed),
                    Err(selected_instructions_to_register_homes::HomedSpillPseudoInstructionError::NonCanonicalFunctions),
                );
            }
            let mut usage = canonical;
            usage.usage.iterations += 1;
            assert_eq!(
                validate(&source, usage),
                Err(selected_instructions_to_register_homes::HomedSpillPseudoInstructionError::UsageMismatch),
            );
        }
    }
}

#[test]
fn exact_budgets_all_five_axes_and_cross_target_custody_fail_closed() {
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
            let source = build(constructor, target);
            assert!(lower(&source, exact).is_ok());
            for actual in insufficient {
                assert_eq!(
                    lower(&source, actual),
                    Err(
                        selected_instructions_to_register_homes::HomedSpillPseudoInstructionError::BudgetExceeded {
                            required: usage,
                            budget: actual
                        }
                    ),
                );
            }
        }
        let x86 = build(constructor, NativeTarget::linux_x64());
        let arm = build(constructor, NativeTarget::linux_arm64());
        assert_eq!(
            selected_instructions_to_register_homes::lower_homed_recursive_spill_pseudos(
                &x86.pseudos,
                &arm.homes,
                selected_instructions_to_register_homes::HomedSpillPseudoInstructionPolicy::RecursiveLogicalScheduleWithClosedReloadHomesV2,
                exact,
            ),
            Err(selected_instructions_to_register_homes::HomedSpillPseudoInstructionError::RootMismatch),
        );
        let foreign = lower(&x86, exact).unwrap().plan().clone();
        assert_eq!(
            validate(&arm, foreign),
            Err(selected_instructions_to_register_homes::HomedSpillPseudoInstructionError::RootMismatch),
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
        validation_steps: if original { 17 } else { 16 },
        commits: if original { 11 } else { 10 },
        iterations: 10,
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
