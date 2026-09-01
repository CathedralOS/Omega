//! Canonical identity for generalized abstract spill insertion.

use sha2::{Digest, Sha256};

use crate::{
    GeneralizedSpillActionSource, GeneralizedSpillEvent, GeneralizedSpillInsertionIdentity,
    GeneralizedSpillInsertionPlan, GeneralizedSpillInsertionPolicy, LogicalSpillStorageClass,
};

pub fn generalized_spill_insertion_identity(
    plan: &GeneralizedSpillInsertionPlan,
) -> GeneralizedSpillInsertionIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.generalized-spill-insertion.v1\0");
    bytes.extend_from_slice(&plan.abstract_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.spill_recovery_actions.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        GeneralizedSpillInsertionPolicy::EpochZeroAndOneBlockLocalUnsignedU64ClosedIntervalFirstFitV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.extend_from_slice(&function.spill_area_bytes.to_le_bytes());
        length(&mut bytes, function.slots.len());
        for slot in &function.slots {
            action_id(&mut bytes, slot.action);
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
                GeneralizedSpillEvent::Store {
                    action,
                    point,
                    before_instruction,
                    before_reload,
                    source,
                    source_view,
                    slot,
                } => {
                    bytes.push(0);
                    action_id(&mut bytes, action);
                    bytes.extend_from_slice(&point.0.to_le_bytes());
                    bytes.extend_from_slice(&before_instruction.0.to_le_bytes());
                    option_action(&mut bytes, before_reload);
                    bytes.extend_from_slice(&source.0.to_le_bytes());
                    bytes.extend_from_slice(&source_view.0.to_le_bytes());
                    action_id(&mut bytes, slot);
                }
                GeneralizedSpillEvent::Reload {
                    action,
                    point,
                    before_instruction,
                    slot,
                    result,
                    destination_class,
                } => {
                    bytes.push(1);
                    action_id(&mut bytes, action);
                    bytes.extend_from_slice(&point.0.to_le_bytes());
                    bytes.extend_from_slice(&before_instruction.0.to_le_bytes());
                    action_id(&mut bytes, slot);
                    action_id(&mut bytes, result);
                    bytes.extend_from_slice(&destination_class.0.to_le_bytes());
                }
                GeneralizedSpillEvent::Rewrite {
                    action,
                    block,
                    point,
                    instruction,
                    operand,
                    result,
                } => {
                    bytes.push(2);
                    action_id(&mut bytes, action);
                    bytes.extend_from_slice(&block.0.to_le_bytes());
                    bytes.extend_from_slice(&point.0.to_le_bytes());
                    bytes.extend_from_slice(&instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&operand.to_le_bytes());
                    action_id(&mut bytes, result);
                }
            }
        }
    }
    GeneralizedSpillInsertionIdentity(Sha256::digest(bytes).into())
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

fn action_id(bytes: &mut Vec<u8>, action: crate::GeneralizedSpillActionId) {
    bytes.extend_from_slice(&action.epoch.to_le_bytes());
    bytes.extend_from_slice(&action.ordinal.to_le_bytes());
}

fn option_action(bytes: &mut Vec<u8>, action: Option<crate::GeneralizedSpillActionId>) {
    match action {
        None => bytes.push(0),
        Some(action) => {
            bytes.push(1);
            action_id(bytes, action);
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
            .expect("generalized spill-insertion length fits u64")
            .to_le_bytes(),
    );
}
