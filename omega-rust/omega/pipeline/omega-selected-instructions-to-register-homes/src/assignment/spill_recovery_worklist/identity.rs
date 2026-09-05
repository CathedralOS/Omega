use sha2::{Digest, Sha256};

use crate::{
    ReloadValueHomePolicy, SpillRecoveryWorklistIdentity, SpillRecoveryWorklistPlan,
    SpillRecoveryWorklistPolicy,
};

pub fn spill_recovery_worklist_identity(
    plan: &SpillRecoveryWorklistPlan,
) -> SpillRecoveryWorklistIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.spill-recovery-worklist.v1\0");
    bytes.extend_from_slice(&plan.abstract_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.logical_spill_operations.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.reload_home_policy {
        ReloadValueHomePolicy::BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1 => 0,
    });
    bytes.extend_from_slice(&plan.reload_home_budget.encode());
    bytes.push(match plan.policy {
        SpillRecoveryWorklistPolicy::SingleReloadPressureEpochOneV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.epochs.len());
    for epoch in &plan.epochs {
        bytes.extend_from_slice(&epoch.epoch.to_le_bytes());
        length(&mut bytes, epoch.work_items.len());
        for item in &epoch.work_items {
            bytes.extend_from_slice(&item.synthetic.epoch.to_le_bytes());
            bytes.extend_from_slice(&item.synthetic.ordinal.to_le_bytes());
            bytes.extend_from_slice(&item.machine.get().to_le_bytes());
            bytes.extend_from_slice(&item.source_reload.0.to_le_bytes());
            bytes.extend_from_slice(&item.block.0.to_le_bytes());
            bytes.extend_from_slice(&item.start.0.to_le_bytes());
            bytes.extend_from_slice(&item.exclusive_end.0.to_le_bytes());
            bytes.extend_from_slice(&item.class.0.to_le_bytes());
            length(&mut bytes, item.candidates.len());
            for candidate in &item.candidates {
                bytes.extend_from_slice(&candidate.0.to_le_bytes());
            }
        }
    }
    SpillRecoveryWorklistIdentity(Sha256::digest(bytes).into())
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("spill-recovery worklist length fits u64")
            .to_le_bytes(),
    );
}
