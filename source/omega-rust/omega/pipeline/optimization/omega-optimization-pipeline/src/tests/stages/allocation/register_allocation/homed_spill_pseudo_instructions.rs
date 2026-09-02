//! V2 target-neutral spill pseudos with exact reload destination views.

use crate::tests::*;
use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use super::recursive_reload_value_homes::{Bundle, original_bundle, reload_bundle};

pub(super) struct HomedBundle {
    pub(super) bundle: Bundle,
    pub(super) pseudos: omega_regalloc::ValidatedSpillPseudoInstructions,
    pub(super) homes: omega_regalloc::ValidatedRecursiveReloadValueHomes,
}

pub(super) fn build(constructor: fn(NativeTarget) -> Bundle, target: NativeTarget) -> HomedBundle {
    let bundle = constructor(target);
    let pseudos = omega_regalloc::lower_recursive_spill_pseudos(
        &bundle.recursive,
        omega_regalloc::SpillPseudoInstructionPolicy::RecursiveLogicalScheduleV1,
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
    omega_regalloc::ValidatedHomedSpillPseudoInstructions,
    omega_regalloc::HomedSpillPseudoInstructionError,
> {
    omega_regalloc::lower_homed_recursive_spill_pseudos(
        &source.pseudos,
        &source.homes,
        omega_regalloc::HomedSpillPseudoInstructionPolicy::RecursiveLogicalScheduleWithClosedReloadHomesV2,
        budget,
    )
}

fn validate(
    source: &HomedBundle,
    plan: omega_regalloc::HomedSpillPseudoInstructionPlan,
) -> Result<
    omega_regalloc::ValidatedHomedSpillPseudoInstructions,
    omega_regalloc::HomedSpillPseudoInstructionError,
> {
    omega_regalloc::validate_homed_spill_pseudo_instructions(&source.pseudos, &source.homes, plan)
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
                    .map(omega_regalloc::HomedSpillPseudoInstruction::id)
                    .map(|id| id.ordinal)
                    .collect::<Vec<_>>(),
                vec![0, 1, 2, 3, 4, 5],
            );
            let reloads = function
                .instructions
                .iter()
                .filter_map(|instruction| match instruction {
                    omega_regalloc::HomedSpillPseudoInstruction::Reload {
                        result,
                        destination_view,
                        ..
                    } => Some((*result, *destination_view)),
                    _ => None,
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
                    omega_regalloc::HomedSpillPseudoInstruction::Store {
                        action,
                        before_reload: Some(omega_regalloc::SpillPseudoInstructionId { ordinal: 4 }),
                        source: omega_regalloc::SpillPseudoStoredValue::Reload { action: source, producer: omega_regalloc::SpillPseudoInstructionId { ordinal: 2 } },
                        ..
                    } if !original && action == id(2, 0) && source == id(0, 0)
                ) || matches!(
                    function.instructions[3],
                    omega_regalloc::HomedSpillPseudoInstruction::Store {
                        action,
                        before_reload: Some(omega_regalloc::SpillPseudoInstructionId { ordinal: 4 }),
                        source: omega_regalloc::SpillPseudoStoredValue::Original(
                            omega_selected_instructions::VirtualRegisterId(5)
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
    let v1 = omega_regalloc::lower_recursive_spill_pseudos(
        &source.bundle.recursive,
        omega_regalloc::SpillPseudoInstructionPolicy::RecursiveLogicalScheduleV1,
        OptimizationWorkBudget::new(1, 6, 23, 10, 14).unwrap(),
    )
    .unwrap();
    assert_eq!(
        v1.receipt().identity().bytes(),
        [
            79, 200, 52, 203, 184, 85, 65, 90, 222, 153, 154, 152, 200, 63, 17, 170, 113, 249, 43,
            17, 54, 212, 54, 21, 191, 231, 157, 14, 147, 181, 100, 84,
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
                |plan: &mut omega_regalloc::HomedSpillPseudoInstructionPlan| {
                    plan.spill_pseudo_instructions =
                        omega_regalloc::SpillPseudoInstructionPlanIdentity::from_bytes([0xc0; 32]);
                },
                |plan: &mut omega_regalloc::HomedSpillPseudoInstructionPlan| {
                    plan.recursive_reload_value_homes =
                        omega_regalloc::RecursiveReloadValueHomeIdentity::from_bytes([0xc1; 32]);
                },
                |plan: &mut omega_regalloc::HomedSpillPseudoInstructionPlan| {
                    plan.register_environment =
                        omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes(
                            [0xc2; 32],
                        );
                },
                |plan: &mut omega_regalloc::HomedSpillPseudoInstructionPlan| {
                    plan.allocator_availability =
                        omega_regalloc::AllocatorAvailabilityIdentity::from_bytes([0xc3; 32]);
                },
                |plan: &mut omega_regalloc::HomedSpillPseudoInstructionPlan| {
                    plan.optimization_unit =
                        omega_optimization_core::OptimizationUnitIdentity::from_bytes([0xc4; 32]);
                },
                |plan: &mut omega_regalloc::HomedSpillPseudoInstructionPlan| {
                    plan.fuel_schedule = psi_core::FuelScheduleIdentity::new(99_990).unwrap();
                },
            ] {
                let mut changed = canonical.clone();
                corrupt(&mut changed);
                assert_eq!(
                    validate(&source, changed),
                    Err(omega_regalloc::HomedSpillPseudoInstructionError::RootMismatch),
                );
            }

            let mutations: [fn(&mut omega_regalloc::HomedSpillPseudoInstructionPlan); 6] = [
                |plan| plan.functions[0].storage[2].spill_area_offset += 8,
                |plan| match &mut plan.functions[0].instructions[0] {
                    omega_regalloc::HomedSpillPseudoInstruction::Store { source_view, .. } => {
                        source_view.0 += 1
                    }
                    _ => unreachable!(),
                },
                |plan| match &mut plan.functions[0].instructions[2] {
                    omega_regalloc::HomedSpillPseudoInstruction::Reload {
                        destination_class,
                        ..
                    } => destination_class.0 += 1,
                    _ => unreachable!(),
                },
                |plan| match &mut plan.functions[0].instructions[2] {
                    omega_regalloc::HomedSpillPseudoInstruction::Reload {
                        destination_view,
                        ..
                    } => destination_view.0 += 1,
                    _ => unreachable!(),
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
                    Err(omega_regalloc::HomedSpillPseudoInstructionError::NonCanonicalFunctions),
                );
            }
            let mut usage = canonical;
            usage.usage.iterations += 1;
            assert_eq!(
                validate(&source, usage),
                Err(omega_regalloc::HomedSpillPseudoInstructionError::UsageMismatch),
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
                        omega_regalloc::HomedSpillPseudoInstructionError::BudgetExceeded {
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
            omega_regalloc::lower_homed_recursive_spill_pseudos(
                &x86.pseudos,
                &arm.homes,
                omega_regalloc::HomedSpillPseudoInstructionPolicy::RecursiveLogicalScheduleWithClosedReloadHomesV2,
                exact,
            ),
            Err(omega_regalloc::HomedSpillPseudoInstructionError::RootMismatch),
        );
        let foreign = lower(&x86, exact).unwrap().plan().clone();
        assert_eq!(
            validate(&arm, foreign),
            Err(omega_regalloc::HomedSpillPseudoInstructionError::RootMismatch),
        );
    }
}

const fn id(epoch: u32, ordinal: u32) -> omega_regalloc::GeneralizedSpillActionId {
    omega_regalloc::GeneralizedSpillActionId { epoch, ordinal }
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
