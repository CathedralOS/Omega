use crate::tests::*;

use super::{
    super::recursive_reload_value_homes::{Bundle, original_bundle, reload_bundle},
    fixture::{build, exact_budget, lower, validate},
};

#[test]
fn independent_replay_rejects_roots_geometry_views_lineage_and_order() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for constructor in [
            reload_bundle as fn(NativeTarget) -> Bundle,
            original_bundle as fn(NativeTarget) -> Bundle,
        ] {
            let source = build(constructor, target);
            let canonical = lower(&source, exact_budget()).unwrap().plan().clone();
            let identity =
                selected_instructions_to_register_homes::abstract_spill_memory_effect_plan_identity(
                    &canonical,
                );
            for corrupt in ROOT_MUTATIONS {
                let mut changed = canonical.clone();
                corrupt(&mut changed);
                assert_ne!(
                    selected_instructions_to_register_homes::abstract_spill_memory_effect_plan_identity(&changed),
                    identity,
                );
                assert_eq!(
                    validate(&source, changed),
                    Err(selected_instructions_to_register_homes::AbstractSpillMemoryEffectError::RootMismatch),
                );
            }
            for corrupt in EFFECT_MUTATIONS {
                let mut changed = canonical.clone();
                corrupt(&mut changed);
                assert_ne!(
                    selected_instructions_to_register_homes::abstract_spill_memory_effect_plan_identity(&changed),
                    identity,
                );
                assert_eq!(
                    validate(&source, changed),
                    Err(selected_instructions_to_register_homes::AbstractSpillMemoryEffectError::NonCanonicalFunctions),
                );
            }
            let mut usage = canonical;
            usage.usage.validation_steps += 1;
            assert_ne!(
                selected_instructions_to_register_homes::abstract_spill_memory_effect_plan_identity(
                    &usage
                ),
                identity,
            );
            assert_eq!(
                validate(&source, usage),
                Err(selected_instructions_to_register_homes::AbstractSpillMemoryEffectError::UsageMismatch),
            );
        }
    }
}

const ROOT_MUTATIONS: [fn(
    &mut selected_instructions_to_register_homes::AbstractSpillMemoryEffectPlan,
); 5] = [
    |plan| {
        plan.homed_spill_pseudo_instructions =
            selected_instructions_to_register_homes::HomedSpillPseudoInstructionPlanIdentity::from_bytes([0xd0; 32]);
    },
    |plan| {
        plan.register_environment =
            register_model::TargetRegisterEnvironmentIdentity::from_bytes([0xd1; 32]);
    },
    |plan| {
        plan.allocator_availability =
            selected_instructions_to_register_homes::AllocatorAvailabilityIdentity::from_bytes(
                [0xd2; 32],
            );
    },
    |plan| {
        plan.optimization_unit =
            optimization_core::OptimizationUnitIdentity::from_bytes([0xd3; 32]);
    },
    |plan| plan.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(99_991).unwrap(),
];

const EFFECT_MUTATIONS: [fn(
    &mut selected_instructions_to_register_homes::AbstractSpillMemoryEffectPlan,
); 9] = [
    |plan| match &mut plan.functions[0].effects[0] {
        selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Write {
            source_view,
            ..
        } => source_view.0 += 1,
        _ => unreachable!(),
    },
    |plan| match &mut plan.functions[0].effects[0] {
        selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Write {
            spill_area_offset,
            ..
        } => *spill_area_offset += 8,
        _ => unreachable!(),
    },
    |plan| match &mut plan.functions[0].effects[0] {
        selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Write {
            size_bytes,
            ..
        } => *size_bytes += 8,
        _ => unreachable!(),
    },
    |plan| match &mut plan.functions[0].effects[0] {
        selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Write {
            alignment_bytes,
            ..
        } => *alignment_bytes *= 2,
        _ => unreachable!(),
    },
    |plan| match &mut plan.functions[0].effects[2] {
        selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Read {
            destination_class,
            ..
        } => destination_class.0 += 1,
        _ => unreachable!(),
    },
    |plan| match &mut plan.functions[0].effects[2] {
        selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Read {
            destination_view,
            ..
        } => destination_view.0 += 1,
        _ => unreachable!(),
    },
    |plan| match &mut plan.functions[0].effects[0] {
        selected_instructions_to_register_homes::AbstractSpillMemoryEffect::Write {
            source,
            ..
        } => {
            *source = selected_instructions_to_register_homes::SpillPseudoStoredValue::Original(
                selected_instructions::VirtualRegisterId(99),
            );
        }
        _ => unreachable!(),
    },
    |plan| plan.functions[0].effects.swap(0, 1),
    |plan| plan.functions[0].spill_area_bytes += 8,
];
