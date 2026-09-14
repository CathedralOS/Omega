//! Canonical V1 identity for abstract spill-area effects.

use sha2::{Digest, Sha256};

use crate::{
    AbstractSpillMemoryEffect, AbstractSpillMemoryEffectPlan,
    AbstractSpillMemoryEffectPlanIdentity, AbstractSpillMemoryEffectPolicy,
    LogicalSpillStorageClass, SpillPseudoStoredValue,
};

pub fn abstract_spill_memory_effect_plan_identity(
    plan: &AbstractSpillMemoryEffectPlan,
) -> AbstractSpillMemoryEffectPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.abstract-spill-memory-effects.v1\0");
    bytes.extend_from_slice(&plan.homed_spill_pseudo_instructions.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        AbstractSpillMemoryEffectPolicy::HomedPseudoReadWriteV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.extend_from_slice(&function.spill_area_bytes.to_le_bytes());
        length(&mut bytes, function.effects.len());
        for effect in &function.effects {
            encode_effect(&mut bytes, *effect);
        }
    }
    AbstractSpillMemoryEffectPlanIdentity(Sha256::digest(bytes).into())
}

fn encode_effect(bytes: &mut Vec<u8>, effect: AbstractSpillMemoryEffect) {
    match effect {
        AbstractSpillMemoryEffect::Write {
            pseudo,
            action: spill_action,
            block,
            point,
            before_instruction,
            before_reload,
            source,
            source_view,
            storage,
            storage_class,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
        } => {
            bytes.push(0);
            bytes.extend_from_slice(&pseudo.ordinal.to_le_bytes());
            action(bytes, spill_action);
            bytes.extend_from_slice(&block.0.to_le_bytes());
            bytes.extend_from_slice(&point.0.to_le_bytes());
            bytes.extend_from_slice(&before_instruction.0.to_le_bytes());
            option_pseudo(bytes, before_reload);
            stored_value(bytes, source);
            bytes.extend_from_slice(&source_view.0.to_le_bytes());
            storage_geometry(
                bytes,
                storage,
                storage_class,
                spill_area_offset,
                size_bytes,
                alignment_bytes,
            );
        }
        AbstractSpillMemoryEffect::Read {
            pseudo,
            action: spill_action,
            block,
            point,
            before_instruction,
            storage,
            storage_class,
            spill_area_offset,
            size_bytes,
            alignment_bytes,
            result,
            destination_class,
            destination_view,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&pseudo.ordinal.to_le_bytes());
            action(bytes, spill_action);
            bytes.extend_from_slice(&block.0.to_le_bytes());
            bytes.extend_from_slice(&point.0.to_le_bytes());
            bytes.extend_from_slice(&before_instruction.0.to_le_bytes());
            storage_geometry(
                bytes,
                storage,
                storage_class,
                spill_area_offset,
                size_bytes,
                alignment_bytes,
            );
            action(bytes, result);
            bytes.extend_from_slice(&destination_class.0.to_le_bytes());
            bytes.extend_from_slice(&destination_view.0.to_le_bytes());
        }
    }
}

fn storage_geometry(
    bytes: &mut Vec<u8>,
    storage: crate::GeneralizedSpillActionId,
    class: LogicalSpillStorageClass,
    offset: u64,
    size: u64,
    alignment: u64,
) {
    action(bytes, storage);
    bytes.push(match class {
        LogicalSpillStorageClass::NonAddressUnsignedU64V1 => 0,
    });
    bytes.extend_from_slice(&offset.to_le_bytes());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.extend_from_slice(&alignment.to_le_bytes());
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
            bytes.extend_from_slice(&producer.ordinal.to_le_bytes());
        }
    }
}

fn action(bytes: &mut Vec<u8>, id: crate::GeneralizedSpillActionId) {
    bytes.extend_from_slice(&id.epoch.to_le_bytes());
    bytes.extend_from_slice(&id.ordinal.to_le_bytes());
}

fn option_pseudo(bytes: &mut Vec<u8>, id: Option<crate::SpillPseudoInstructionId>) {
    match id {
        None => bytes.push(0),
        Some(id) => {
            bytes.push(1);
            bytes.extend_from_slice(&id.ordinal.to_le_bytes());
        }
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("abstract spill-effect length fits u64")
            .to_le_bytes(),
    );
}
