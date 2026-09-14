//! Canonical identity for epoch-two generalized victim-choice custody.

use sha2::{Digest, Sha256};

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedSpillRecoveryChoiceIdentity,
    GeneralizedSpillRecoveryChoicePlan, GeneralizedSpillRecoveryChoicePolicy,
};

pub fn generalized_spill_recovery_choice_identity(
    plan: &GeneralizedSpillRecoveryChoicePlan,
) -> GeneralizedSpillRecoveryChoiceIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.generalized-spill-recovery-choice.v2\0");
    bytes.extend_from_slice(&plan.worklist.bytes());
    bytes.extend_from_slice(&plan.reload_value_homes.bytes());
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1 => 0,
        GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1 => 1,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.choices.len());
    for choice in &plan.choices {
        bytes.extend_from_slice(&choice.work_item.epoch.to_le_bytes());
        bytes.extend_from_slice(&choice.work_item.ordinal.to_le_bytes());
        length(&mut bytes, choice.function);
        bytes.extend_from_slice(&choice.machine.get().to_le_bytes());
        bytes.extend_from_slice(&choice.block.0.to_le_bytes());
        bytes.extend_from_slice(&choice.point.0.to_le_bytes());
        bytes.extend_from_slice(&choice.source_pressure.epoch.to_le_bytes());
        bytes.extend_from_slice(&choice.source_pressure.ordinal.to_le_bytes());
        bytes.extend_from_slice(&choice.reload_class.0.to_le_bytes());
        length(&mut bytes, choice.reload_candidates.len());
        for candidate in &choice.reload_candidates {
            bytes.extend_from_slice(&candidate.0.to_le_bytes());
        }
        length(&mut bytes, choice.blocking_residents.len());
        for resident in &choice.blocking_residents {
            value(&mut bytes, resident.value);
            bytes.extend_from_slice(&resident.class.0.to_le_bytes());
            bytes.extend_from_slice(&resident.start.0.to_le_bytes());
            bytes.extend_from_slice(&resident.exclusive_end.0.to_le_bytes());
            bytes.extend_from_slice(&resident.view.0.to_le_bytes());
        }
        length(&mut bytes, choice.contenders.len());
        for contender in &choice.contenders {
            value(&mut bytes, contender.value);
            bytes.extend_from_slice(&contender.exclusive_end.0.to_le_bytes());
            bytes.extend_from_slice(&contender.resident_view.0.to_le_bytes());
            bytes.extend_from_slice(&contender.reclaimed_view.0.to_le_bytes());
        }
        value(&mut bytes, choice.selected_victim);
        bytes.extend_from_slice(&choice.selected_victim_view.0.to_le_bytes());
        bytes.extend_from_slice(&choice.reclaimed_view.0.to_le_bytes());
    }
    GeneralizedSpillRecoveryChoiceIdentity(Sha256::digest(bytes).into())
}

fn value(bytes: &mut Vec<u8>, value: GeneralizedReloadCoexistingValue) {
    match value {
        GeneralizedReloadCoexistingValue::Original(register) => {
            bytes.push(0);
            bytes.extend_from_slice(&register.0.to_le_bytes());
        }
        GeneralizedReloadCoexistingValue::Reload(action) => {
            bytes.push(1);
            bytes.extend_from_slice(&action.epoch.to_le_bytes());
            bytes.extend_from_slice(&action.ordinal.to_le_bytes());
        }
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("generalized spill-recovery choice length fits u64")
            .to_le_bytes(),
    );
}
