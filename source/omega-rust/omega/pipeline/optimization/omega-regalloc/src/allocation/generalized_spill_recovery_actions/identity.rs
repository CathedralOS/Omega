//! Canonical identity for epoch-two logical recovery-action custody.

use sha2::{Digest, Sha256};

use crate::{
    GeneralizedSpillRecoveryActionIdentity, GeneralizedSpillRecoveryActionPlan,
    GeneralizedSpillRecoveryActionPolicy,
};

pub fn generalized_spill_recovery_action_identity(
    plan: &GeneralizedSpillRecoveryActionPlan,
) -> GeneralizedSpillRecoveryActionIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.generalized-spill-recovery-actions.v1\0");
    bytes.extend_from_slice(&plan.generalized_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.reload_value_homes.bytes());
    bytes.extend_from_slice(&plan.choices.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        GeneralizedSpillRecoveryActionPolicy::EpochTwoReloadVictimLaterGeneralizedRewritesV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.actions.len());
    for action in &plan.actions {
        bytes.extend_from_slice(&action.source_work_item.epoch.to_le_bytes());
        bytes.extend_from_slice(&action.source_work_item.ordinal.to_le_bytes());
        length(&mut bytes, action.function);
        bytes.extend_from_slice(&action.machine.get().to_le_bytes());
        bytes.extend_from_slice(&action.block.0.to_le_bytes());
        bytes.extend_from_slice(&action.pressure_point.0.to_le_bytes());
        action_id(&mut bytes, action.source_pressure);
        action_id(&mut bytes, action.victim);
        bytes.extend_from_slice(&action.victim_class.0.to_le_bytes());
        bytes.extend_from_slice(&action.current_view.0.to_le_bytes());
        bytes.extend_from_slice(&action.reclaimed_view.0.to_le_bytes());
        action_id(&mut bytes, action.storage.id);
        bytes.push(action.storage.class as u8);
        action_id(&mut bytes, action.store.before_pressure_reload);
        bytes.extend_from_slice(&action.store.before_instruction.0.to_le_bytes());
        action_id(&mut bytes, action.store.source);
        bytes.extend_from_slice(&action.store.source_view.0.to_le_bytes());
        action_id(&mut bytes, action.store.storage);
        bytes.extend_from_slice(&action.reload.before_instruction.0.to_le_bytes());
        action_id(&mut bytes, action.reload.storage);
        action_id(&mut bytes, action.reload.result);
        bytes.extend_from_slice(&action.reload.destination_class.0.to_le_bytes());
        length(&mut bytes, action.rewrites.len());
        for rewrite in &action.rewrites {
            bytes.extend_from_slice(&rewrite.block.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.point.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.instruction.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.operand.to_le_bytes());
            action_id(&mut bytes, rewrite.result);
        }
    }
    GeneralizedSpillRecoveryActionIdentity(Sha256::digest(bytes).into())
}

fn action_id(bytes: &mut Vec<u8>, action: crate::GeneralizedSpillActionId) {
    bytes.extend_from_slice(&action.epoch.to_le_bytes());
    bytes.extend_from_slice(&action.ordinal.to_le_bytes());
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("generalized spill-recovery action length fits u64")
            .to_le_bytes(),
    );
}
