use sha2::{Digest, Sha256};

use crate::{SpillRecoveryChoiceIdentity, SpillRecoveryChoicePlan, SpillRecoveryChoicePolicy};

pub fn spill_recovery_choice_identity(
    plan: &SpillRecoveryChoicePlan,
) -> SpillRecoveryChoiceIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.spill-recovery-choice.v1\0");
    bytes.extend_from_slice(&plan.worklist.bytes());
    bytes.extend_from_slice(&plan.abstract_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.push(match plan.policy {
        SpillRecoveryChoicePolicy::EpochOneFarthestEndThenHighestVregV1 => 0,
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
        bytes.extend_from_slice(&choice.reload_class.0.to_le_bytes());
        length(&mut bytes, choice.reload_candidates.len());
        for view in &choice.reload_candidates {
            bytes.extend_from_slice(&view.0.to_le_bytes());
        }
        length(&mut bytes, choice.active_residents.len());
        for resident in &choice.active_residents {
            bytes.extend_from_slice(&resident.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&resident.class.0.to_le_bytes());
            bytes.extend_from_slice(&resident.start.0.to_le_bytes());
            bytes.extend_from_slice(&resident.exclusive_end.0.to_le_bytes());
            bytes.extend_from_slice(&resident.view.0.to_le_bytes());
        }
        length(&mut bytes, choice.contenders.len());
        for contender in &choice.contenders {
            bytes.extend_from_slice(&contender.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&contender.exclusive_end.0.to_le_bytes());
            bytes.extend_from_slice(&contender.resident_view.0.to_le_bytes());
            bytes.extend_from_slice(&contender.reclaimed_view.0.to_le_bytes());
        }
        bytes.extend_from_slice(&choice.selected_victim.0.to_le_bytes());
        bytes.extend_from_slice(&choice.selected_victim_view.0.to_le_bytes());
        bytes.extend_from_slice(&choice.reclaimed_view.0.to_le_bytes());
    }
    SpillRecoveryChoiceIdentity(Sha256::digest(bytes).into())
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("spill-recovery choice length fits u64")
            .to_le_bytes(),
    );
}
