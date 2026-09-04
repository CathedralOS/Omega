//! Canonical identity for generalized spill-recovery work custody.

use sha2::{Digest, Sha256};

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    GeneralizedSpillRecoveryWorklistIdentity, GeneralizedSpillRecoveryWorklistPlan,
    GeneralizedSpillRecoveryWorklistPolicy,
};

pub fn generalized_spill_recovery_worklist_identity(
    plan: &GeneralizedSpillRecoveryWorklistPlan,
) -> GeneralizedSpillRecoveryWorklistIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.generalized-spill-recovery-worklist.v1\0");
    bytes.extend_from_slice(&plan.reload_value_homes.bytes());
    bytes.extend_from_slice(&plan.generalized_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.abstract_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.spill_recovery_actions.bytes());
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        match &function.item {
            None => bytes.push(0),
            Some(item) => {
                bytes.push(1);
                bytes.extend_from_slice(&item.id.epoch.to_le_bytes());
                bytes.extend_from_slice(&item.id.ordinal.to_le_bytes());
                action(&mut bytes, item.source_pressure);
                source(&mut bytes, item.source);
                bytes.extend_from_slice(&item.block.0.to_le_bytes());
                bytes.extend_from_slice(&item.start.0.to_le_bytes());
                bytes.extend_from_slice(&item.exclusive_end.0.to_le_bytes());
                bytes.extend_from_slice(&item.class.0.to_le_bytes());
                length(&mut bytes, item.candidates.len());
                for candidate in &item.candidates {
                    bytes.extend_from_slice(&candidate.0.to_le_bytes());
                }
                length(&mut bytes, item.blocking_homes.len());
                for home in &item.blocking_homes {
                    match home.value {
                        GeneralizedReloadCoexistingValue::Original(register) => {
                            bytes.push(0);
                            bytes.extend_from_slice(&register.0.to_le_bytes());
                        }
                        GeneralizedReloadCoexistingValue::Reload(reload) => {
                            bytes.push(1);
                            action(&mut bytes, reload);
                        }
                    }
                    bytes.extend_from_slice(&home.class.0.to_le_bytes());
                    bytes.extend_from_slice(&home.view.0.to_le_bytes());
                }
            }
        }
    }
    GeneralizedSpillRecoveryWorklistIdentity(Sha256::digest(bytes).into())
}

fn action(bytes: &mut Vec<u8>, action: GeneralizedSpillActionId) {
    bytes.extend_from_slice(&action.epoch.to_le_bytes());
    bytes.extend_from_slice(&action.ordinal.to_le_bytes());
}

fn source(bytes: &mut Vec<u8>, source: GeneralizedSpillActionSource) {
    match source {
        GeneralizedSpillActionSource::EpochZero { storage, reload } => {
            bytes.push(0);
            bytes.extend_from_slice(&storage.0.to_le_bytes());
            bytes.extend_from_slice(&reload.0.to_le_bytes());
        }
        GeneralizedSpillActionSource::EpochOne {
            work_item,
            storage,
            source_reload,
            reload,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&work_item.epoch.to_le_bytes());
            bytes.extend_from_slice(&work_item.ordinal.to_le_bytes());
            bytes.extend_from_slice(&storage.epoch.to_le_bytes());
            bytes.extend_from_slice(&storage.ordinal.to_le_bytes());
            bytes.extend_from_slice(&source_reload.0.to_le_bytes());
            bytes.extend_from_slice(&reload.epoch.to_le_bytes());
            bytes.extend_from_slice(&reload.ordinal.to_le_bytes());
        }
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("generalized recovery worklist length fits u64")
            .to_le_bytes(),
    );
}
