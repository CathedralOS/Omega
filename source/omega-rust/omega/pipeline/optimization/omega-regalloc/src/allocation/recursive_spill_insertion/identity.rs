//! Canonical identity for the recursive logical spill schedule.

use sha2::{Digest, Sha256};

use crate::{
    GeneralizedSpillActionSource, LogicalSpillStorageClass, RecursiveSpillActionSource,
    RecursiveSpillEvent, RecursiveSpillInsertionIdentity, RecursiveSpillInsertionPlan,
    RecursiveSpillInsertionPolicy, RecursiveSpillStoredValue,
};

pub fn recursive_spill_insertion_identity(
    plan: &RecursiveSpillInsertionPlan,
) -> RecursiveSpillInsertionIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(match plan.policy {
        RecursiveSpillInsertionPolicy::EpochTwoReloadVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV1 => {
            b"omega.recursive-spill-insertion.v1\0"
        }
        RecursiveSpillInsertionPolicy::EpochTwoOriginalVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV2 => {
            b"omega.recursive-spill-insertion.v2\0"
        }
    });
    bytes.extend_from_slice(&plan.generalized_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.recovery_actions.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        RecursiveSpillInsertionPolicy::EpochTwoReloadVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV1 => 0,
        RecursiveSpillInsertionPolicy::EpochTwoOriginalVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV2 => 1,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.extend_from_slice(&function.spill_area_bytes.to_le_bytes());
        length(&mut bytes, function.slots.len());
        for slot in &function.slots {
            action(&mut bytes, slot.action);
            source(&mut bytes, slot.source);
            bytes.push(storage_class(slot.class));
            bytes.extend_from_slice(&slot.block.0.to_le_bytes());
            bytes.extend_from_slice(&slot.live_from.0.to_le_bytes());
            bytes.extend_from_slice(&slot.live_through.0.to_le_bytes());
            bytes.extend_from_slice(&slot.size_bytes.to_le_bytes());
            bytes.extend_from_slice(&slot.alignment_bytes.to_le_bytes());
            bytes.extend_from_slice(&slot.spill_area_offset.to_le_bytes());
        }
        length(&mut bytes, function.schedule.len());
        for event in &function.schedule {
            match *event {
                RecursiveSpillEvent::Store {
                    action: id,
                    point,
                    before_instruction,
                    before_reload,
                    source: stored,
                    source_view,
                    slot,
                } => {
                    bytes.push(0);
                    action(&mut bytes, id);
                    bytes.extend_from_slice(&point.0.to_le_bytes());
                    bytes.extend_from_slice(&before_instruction.0.to_le_bytes());
                    option_action(&mut bytes, before_reload);
                    stored_value(&mut bytes, stored);
                    bytes.extend_from_slice(&source_view.0.to_le_bytes());
                    action(&mut bytes, slot);
                }
                RecursiveSpillEvent::Reload {
                    action: id,
                    point,
                    before_instruction,
                    slot,
                    result,
                    destination_class,
                } => {
                    bytes.push(1);
                    action(&mut bytes, id);
                    bytes.extend_from_slice(&point.0.to_le_bytes());
                    bytes.extend_from_slice(&before_instruction.0.to_le_bytes());
                    action(&mut bytes, slot);
                    action(&mut bytes, result);
                    bytes.extend_from_slice(&destination_class.0.to_le_bytes());
                }
                RecursiveSpillEvent::Rewrite {
                    action: id,
                    block,
                    point,
                    instruction,
                    operand,
                    result,
                } => {
                    bytes.push(2);
                    action(&mut bytes, id);
                    bytes.extend_from_slice(&block.0.to_le_bytes());
                    bytes.extend_from_slice(&point.0.to_le_bytes());
                    bytes.extend_from_slice(&instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&operand.to_le_bytes());
                    action(&mut bytes, result);
                }
            }
        }
    }
    RecursiveSpillInsertionIdentity(Sha256::digest(bytes).into())
}

fn source(bytes: &mut Vec<u8>, source: RecursiveSpillActionSource) {
    match source {
        RecursiveSpillActionSource::Prior(prior) => {
            bytes.push(0);
            prior_source(bytes, prior);
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

fn prior_source(bytes: &mut Vec<u8>, source: GeneralizedSpillActionSource) {
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

fn stored_value(bytes: &mut Vec<u8>, value: RecursiveSpillStoredValue) {
    match value {
        RecursiveSpillStoredValue::Original(register) => {
            bytes.push(0);
            bytes.extend_from_slice(&register.0.to_le_bytes());
        }
        RecursiveSpillStoredValue::Reload(id) => {
            bytes.push(1);
            action(bytes, id);
        }
    }
}

fn action(bytes: &mut Vec<u8>, id: crate::GeneralizedSpillActionId) {
    bytes.extend_from_slice(&id.epoch.to_le_bytes());
    bytes.extend_from_slice(&id.ordinal.to_le_bytes());
}

fn option_action(bytes: &mut Vec<u8>, id: Option<crate::GeneralizedSpillActionId>) {
    match id {
        None => bytes.push(0),
        Some(id) => {
            bytes.push(1);
            action(bytes, id);
        }
    }
}

fn storage_class(class: LogicalSpillStorageClass) -> u8 {
    match class {
        LogicalSpillStorageClass::NonAddressUnsignedU64V1 => 0,
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("recursive spill length fits u64")
            .to_le_bytes(),
    );
}
