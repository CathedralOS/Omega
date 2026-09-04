//! Canonical identity for generalized reload-home evidence.

use sha2::{Digest, Sha256};

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedReloadValueHomeIdentity,
    GeneralizedReloadValueHomeOutcome, GeneralizedReloadValueHomePlan,
    GeneralizedReloadValueHomePolicy, GeneralizedSpillActionSource,
};

pub fn generalized_reload_value_home_identity(
    plan: &GeneralizedReloadValueHomePlan,
) -> GeneralizedReloadValueHomeIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.generalized-reload-value-home.v1\0");
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
        GeneralizedReloadValueHomePolicy::EpochZeroAndOneBlockLocalLowestCompatibleViewV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        length(&mut bytes, function.outcomes.len());
        for outcome in &function.outcomes {
            match outcome {
                GeneralizedReloadValueHomeOutcome::Assigned(assignment) => {
                    bytes.push(0);
                    row(
                        &mut bytes,
                        assignment.result,
                        assignment.source,
                        assignment.block,
                        assignment.start,
                        assignment.exclusive_end,
                        assignment.class,
                        &assignment.candidates,
                    );
                    bytes.extend_from_slice(&assignment.view.0.to_le_bytes());
                    homes(&mut bytes, &assignment.coexisting_homes);
                }
                GeneralizedReloadValueHomeOutcome::Pressure(pressure) => {
                    bytes.push(1);
                    row(
                        &mut bytes,
                        pressure.result,
                        pressure.source,
                        pressure.block,
                        pressure.start,
                        pressure.exclusive_end,
                        pressure.class,
                        &pressure.candidates,
                    );
                    homes(&mut bytes, &pressure.blocking_homes);
                }
            }
        }
    }
    GeneralizedReloadValueHomeIdentity(Sha256::digest(bytes).into())
}

#[allow(clippy::too_many_arguments)]
fn row(
    bytes: &mut Vec<u8>,
    result: crate::GeneralizedSpillActionId,
    origin: GeneralizedSpillActionSource,
    block: omega_selected_instructions::SelectedBlockId,
    start: crate::LiveRangePoint,
    exclusive_end: crate::LiveRangePoint,
    class: omega_register_model::RegisterClassId,
    candidates: &[omega_register_model::RegisterViewId],
) {
    action(bytes, result);
    source(bytes, origin);
    bytes.extend_from_slice(&block.0.to_le_bytes());
    bytes.extend_from_slice(&start.0.to_le_bytes());
    bytes.extend_from_slice(&exclusive_end.0.to_le_bytes());
    bytes.extend_from_slice(&class.0.to_le_bytes());
    length(bytes, candidates.len());
    for candidate in candidates {
        bytes.extend_from_slice(&candidate.0.to_le_bytes());
    }
}

fn homes(bytes: &mut Vec<u8>, values: &[crate::GeneralizedReloadCoexistingHome]) {
    length(bytes, values.len());
    for home in values {
        match home.value {
            GeneralizedReloadCoexistingValue::Original(register) => {
                bytes.push(0);
                bytes.extend_from_slice(&register.0.to_le_bytes());
            }
            GeneralizedReloadCoexistingValue::Reload(result) => {
                bytes.push(1);
                action(bytes, result);
            }
        }
        bytes.extend_from_slice(&home.class.0.to_le_bytes());
        bytes.extend_from_slice(&home.view.0.to_le_bytes());
    }
}

fn action(bytes: &mut Vec<u8>, value: crate::GeneralizedSpillActionId) {
    bytes.extend_from_slice(&value.epoch.to_le_bytes());
    bytes.extend_from_slice(&value.ordinal.to_le_bytes());
}

fn source(bytes: &mut Vec<u8>, value: GeneralizedSpillActionSource) {
    match value {
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
            .expect("generalized reload-home length fits u64")
            .to_le_bytes(),
    );
}
