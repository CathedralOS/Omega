//! Canonical V2 identity for homed target-neutral spill pseudos.

use sha2::{Digest, Sha256};

use crate::{
    HomedSpillPseudoInstruction, HomedSpillPseudoInstructionPlan,
    HomedSpillPseudoInstructionPlanIdentity, HomedSpillPseudoInstructionPolicy,
    LogicalSpillStorageClass, SpillPseudoStoredValue,
};

pub fn homed_spill_pseudo_instruction_plan_identity(
    plan: &HomedSpillPseudoInstructionPlan,
) -> HomedSpillPseudoInstructionPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.homed-spill-pseudo-instructions.v2\0");
    bytes.extend_from_slice(&plan.spill_pseudo_instructions.bytes());
    bytes.extend_from_slice(&plan.recursive_reload_value_homes.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        HomedSpillPseudoInstructionPolicy::RecursiveLogicalScheduleWithClosedReloadHomesV2 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.extend_from_slice(&function.spill_area_bytes.to_le_bytes());
        length(&mut bytes, function.storage.len());
        for storage in &function.storage {
            action(&mut bytes, storage.id);
            bytes.push(storage_class(storage.class));
            bytes.extend_from_slice(&storage.block.0.to_le_bytes());
            bytes.extend_from_slice(&storage.live_from.0.to_le_bytes());
            bytes.extend_from_slice(&storage.live_through.0.to_le_bytes());
            bytes.extend_from_slice(&storage.size_bytes.to_le_bytes());
            bytes.extend_from_slice(&storage.alignment_bytes.to_le_bytes());
            bytes.extend_from_slice(&storage.spill_area_offset.to_le_bytes());
        }
        length(&mut bytes, function.instructions.len());
        for instruction in &function.instructions {
            match *instruction {
                HomedSpillPseudoInstruction::Store {
                    id,
                    action: spill_action,
                    block,
                    point,
                    before_instruction,
                    before_reload,
                    source,
                    source_view,
                    storage,
                } => {
                    bytes.push(0);
                    pseudo(&mut bytes, id);
                    action(&mut bytes, spill_action);
                    bytes.extend_from_slice(&block.0.to_le_bytes());
                    bytes.extend_from_slice(&point.0.to_le_bytes());
                    bytes.extend_from_slice(&before_instruction.0.to_le_bytes());
                    option_pseudo(&mut bytes, before_reload);
                    stored_value(&mut bytes, source);
                    bytes.extend_from_slice(&source_view.0.to_le_bytes());
                    action(&mut bytes, storage);
                }
                HomedSpillPseudoInstruction::Reload {
                    id,
                    action: spill_action,
                    block,
                    point,
                    before_instruction,
                    storage,
                    result,
                    destination_class,
                    destination_view,
                } => {
                    bytes.push(1);
                    pseudo(&mut bytes, id);
                    action(&mut bytes, spill_action);
                    bytes.extend_from_slice(&block.0.to_le_bytes());
                    bytes.extend_from_slice(&point.0.to_le_bytes());
                    bytes.extend_from_slice(&before_instruction.0.to_le_bytes());
                    action(&mut bytes, storage);
                    action(&mut bytes, result);
                    bytes.extend_from_slice(&destination_class.0.to_le_bytes());
                    bytes.extend_from_slice(&destination_view.0.to_le_bytes());
                }
            }
        }
        length(&mut bytes, function.rewrites.len());
        for rewrite in &function.rewrites {
            action(&mut bytes, rewrite.action);
            bytes.extend_from_slice(&rewrite.block.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.point.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.instruction.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.operand.to_le_bytes());
            action(&mut bytes, rewrite.result);
            pseudo(&mut bytes, rewrite.producer);
        }
    }
    HomedSpillPseudoInstructionPlanIdentity(Sha256::digest(bytes).into())
}

fn stored_value(bytes: &mut Vec<u8>, value: SpillPseudoStoredValue) {
    match value {
        SpillPseudoStoredValue::Original(register) => {
            bytes.push(0);
            bytes.extend_from_slice(&register.0.to_le_bytes());
        }
        SpillPseudoStoredValue::Reload {
            action: id,
            producer,
        } => {
            bytes.push(1);
            action(bytes, id);
            pseudo(bytes, producer);
        }
    }
}

fn action(bytes: &mut Vec<u8>, id: crate::GeneralizedSpillActionId) {
    bytes.extend_from_slice(&id.epoch.to_le_bytes());
    bytes.extend_from_slice(&id.ordinal.to_le_bytes());
}

fn pseudo(bytes: &mut Vec<u8>, id: crate::SpillPseudoInstructionId) {
    bytes.extend_from_slice(&id.ordinal.to_le_bytes());
}

fn option_pseudo(bytes: &mut Vec<u8>, id: Option<crate::SpillPseudoInstructionId>) {
    match id {
        None => bytes.push(0),
        Some(id) => {
            bytes.push(1);
            pseudo(bytes, id);
        }
    }
}

const fn storage_class(class: LogicalSpillStorageClass) -> u8 {
    match class {
        LogicalSpillStorageClass::NonAddressUnsignedU64V1 => 0,
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("homed spill-pseudo length fits u64")
            .to_le_bytes(),
    );
}
