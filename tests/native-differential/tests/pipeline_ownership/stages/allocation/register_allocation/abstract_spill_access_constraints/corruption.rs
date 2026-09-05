use crate::tests::*;

use super::{
    super::recursive_reload_value_homes::{Bundle, original_bundle, reload_bundle},
    fixture::{build, constrain, exact_budget, validate},
};

#[test]
fn replay_rejects_every_root_placement_dependency_and_usage_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for constructor in [
            reload_bundle as fn(NativeTarget) -> Bundle,
            original_bundle as fn(NativeTarget) -> Bundle,
        ] {
            let source = build(constructor, target);
            let canonical = constrain(&source, exact_budget()).unwrap().plan().clone();
            let identity =
                omega_selected_instructions_to_register_homes::abstract_spill_access_constraint_plan_identity(&canonical);
            for corrupt in ROOT_MUTATIONS {
                let mut changed = canonical.clone();
                corrupt(&mut changed);
                assert_ne!(
                    omega_selected_instructions_to_register_homes::abstract_spill_access_constraint_plan_identity(&changed),
                    identity
                );
                assert_eq!(
                    validate(&source, changed),
                    Err(omega_selected_instructions_to_register_homes::AbstractSpillAccessConstraintError::RootMismatch),
                );
            }
            for corrupt in CONTENT_MUTATIONS {
                let mut changed = canonical.clone();
                corrupt(&mut changed);
                assert_ne!(
                    omega_selected_instructions_to_register_homes::abstract_spill_access_constraint_plan_identity(&changed),
                    identity
                );
                assert_eq!(
                    validate(&source, changed),
                    Err(omega_selected_instructions_to_register_homes::AbstractSpillAccessConstraintError::NonCanonicalFunctions),
                );
            }
            let mut usage = canonical;
            usage.usage.iterations += 1;
            assert_ne!(
                omega_selected_instructions_to_register_homes::abstract_spill_access_constraint_plan_identity(&usage),
                identity
            );
            assert_eq!(
                validate(&source, usage),
                Err(omega_selected_instructions_to_register_homes::AbstractSpillAccessConstraintError::UsageMismatch),
            );
        }
    }
}

const ROOT_MUTATIONS: [fn(
    &mut omega_selected_instructions_to_register_homes::AbstractSpillAccessConstraintPlan,
); 5] = [
    |plan| {
        plan.abstract_spill_memory_effects =
            omega_selected_instructions_to_register_homes::AbstractSpillMemoryEffectPlanIdentity::from_bytes([0xe0; 32])
    },
    |plan| {
        plan.register_environment =
            omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xe1; 32])
    },
    |plan| {
        plan.allocator_availability =
            omega_selected_instructions_to_register_homes::AllocatorAvailabilityIdentity::from_bytes(
                [0xe2; 32],
            )
    },
    |plan| {
        plan.optimization_unit =
            omega_optimization_core::OptimizationUnitIdentity::from_bytes([0xe3; 32])
    },
    |plan| plan.fuel_schedule = psi_core::FuelScheduleIdentity::new(99_992).unwrap(),
];

const CONTENT_MUTATIONS: [fn(
    &mut omega_selected_instructions_to_register_homes::AbstractSpillAccessConstraintPlan,
); 13] = [
    |plan| plan.functions[0].placements[0].pseudo.ordinal += 1,
    |plan| plan.functions[0].placements[0].block_ordinal += 1,
    |plan| plan.functions[0].placements[0].point.0 += 1,
    |plan| plan.functions[0].placements[0].before_instruction.0 += 1,
    |plan| {
        plan.functions[0].placements[0].kind =
            omega_selected_instructions_to_register_homes::AbstractSpillAccessKind::Read
    },
    |plan| plan.functions[0].placements[0].storage.epoch += 1,
    |plan| plan.functions[0].placements[0].spill_area_offset += 8,
    |plan| plan.functions[0].placements[0].size_bytes += 8,
    |plan| plan.functions[0].placements[0].alignment_bytes *= 2,
    |plan| plan.functions[0].placements.swap(0, 1),
    |plan| plan.functions[0].dependencies[0].after.ordinal += 1,
    |plan| {
        plan.functions[0].dependencies[0].reason =
            omega_selected_instructions_to_register_homes::AbstractSpillAccessDependencyReason::DeclaredBeforeReload;
    },
    |plan| plan.functions[0].dependencies.pop().map(|_| ()).unwrap(),
];
