//! Canonical identity for complete recursive reload-home evidence.

use sha2::{Digest, Sha256};

use crate::{
    RecursiveReloadCoexistingValue, RecursiveReloadValueHomeIdentity, RecursiveReloadValueHomePlan,
    RecursiveReloadValueHomePolicy, RecursiveSpillActionSource,
};

pub fn recursive_reload_value_home_identity(
    plan: &RecursiveReloadValueHomePlan,
) -> RecursiveReloadValueHomeIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.recursive-reload-value-home.v1\0");
    bytes.extend_from_slice(&plan.recursive_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.recovery_actions.bytes());
    bytes.extend_from_slice(&plan.prior_reload_value_homes.bytes());
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        RecursiveReloadValueHomePolicy::CompleteBlockLocalLowestCompatibleViewV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        length(&mut bytes, function.assignments.len());
        for row in &function.assignments {
            action(&mut bytes, row.result);
            source(&mut bytes, row.source);
            bytes.extend_from_slice(&row.block.0.to_le_bytes());
            bytes.extend_from_slice(&row.start.0.to_le_bytes());
            bytes.extend_from_slice(&row.exclusive_end.0.to_le_bytes());
            bytes.extend_from_slice(&row.class.0.to_le_bytes());
            length(&mut bytes, row.candidates.len());
            for candidate in &row.candidates {
                bytes.extend_from_slice(&candidate.0.to_le_bytes());
            }
            bytes.extend_from_slice(&row.view.0.to_le_bytes());
            length(&mut bytes, row.coexisting_homes.len());
            for home in &row.coexisting_homes {
                match home.value {
                    RecursiveReloadCoexistingValue::Original(register) => {
                        bytes.push(0);
                        bytes.extend_from_slice(&register.0.to_le_bytes());
                    }
                    RecursiveReloadCoexistingValue::Reload(result) => {
                        bytes.push(1);
                        action(&mut bytes, result);
                    }
                }
                bytes.extend_from_slice(&home.class.0.to_le_bytes());
                bytes.extend_from_slice(&home.view.0.to_le_bytes());
            }
        }
    }
    RecursiveReloadValueHomeIdentity(Sha256::digest(bytes).into())
}

fn source(bytes: &mut Vec<u8>, value: RecursiveSpillActionSource) {
    match value {
        RecursiveSpillActionSource::Prior(prior) => {
            bytes.push(0);
            match prior {
                crate::GeneralizedSpillActionSource::EpochZero { storage, reload } => {
                    bytes.push(0);
                    bytes.extend_from_slice(&storage.0.to_le_bytes());
                    bytes.extend_from_slice(&reload.0.to_le_bytes());
                }
                crate::GeneralizedSpillActionSource::EpochOne {
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
        RecursiveSpillActionSource::EpochTwo {
            work_item,
            source_pressure,
            victim,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&work_item.epoch.to_le_bytes());
            bytes.extend_from_slice(&work_item.ordinal.to_le_bytes());
            action(bytes, source_pressure);
            action(bytes, victim);
        }
        RecursiveSpillActionSource::EpochTwoOriginal {
            work_item,
            source_pressure,
            victim,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&work_item.epoch.to_le_bytes());
            bytes.extend_from_slice(&work_item.ordinal.to_le_bytes());
            action(bytes, source_pressure);
            bytes.extend_from_slice(&victim.0.to_le_bytes());
        }
    }
}

fn action(bytes: &mut Vec<u8>, value: crate::GeneralizedSpillActionId) {
    bytes.extend_from_slice(&value.epoch.to_le_bytes());
    bytes.extend_from_slice(&value.ordinal.to_le_bytes());
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("recursive reload-home length fits u64")
            .to_le_bytes(),
    );
}
