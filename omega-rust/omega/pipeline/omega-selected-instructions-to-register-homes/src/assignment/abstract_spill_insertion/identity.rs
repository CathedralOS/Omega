//! Stable semantic identity for the abstract insertion schedule.

use sha2::{Digest, Sha256};

use crate::{
    AbstractSpillInsertionIdentity, AbstractSpillInsertionPlan, AbstractSpillInsertionPolicy,
    LogicalSpillStorageClass,
};

pub fn abstract_spill_insertion_identity(
    plan: &AbstractSpillInsertionPlan,
) -> AbstractSpillInsertionIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.abstract-spill-insertion.v1\0");
    bytes.extend_from_slice(&plan.logical_spill_operations.bytes());
    bytes.extend_from_slice(&plan.stack_slot_coloring.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        AbstractSpillInsertionPolicy::BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.extend_from_slice(&function.spill_area_bytes.to_le_bytes());
        let Some(action) = &function.action else {
            bytes.push(0);
            continue;
        };
        bytes.push(1);
        bytes.extend_from_slice(&action.pressure_point.0.to_le_bytes());
        bytes.extend_from_slice(&action.incoming.0.to_le_bytes());
        bytes.extend_from_slice(&action.incoming_view.0.to_le_bytes());
        bytes.extend_from_slice(&action.victim.0.to_le_bytes());
        bytes.extend_from_slice(&action.victim_view.0.to_le_bytes());
        bytes.extend_from_slice(&action.slot.storage.0.to_le_bytes());
        bytes.push(match action.slot.class {
            LogicalSpillStorageClass::NonAddressUnsignedU64V1 => 0,
        });
        bytes.extend_from_slice(&action.slot.size_bytes.to_le_bytes());
        bytes.extend_from_slice(&action.slot.alignment_bytes.to_le_bytes());
        bytes.extend_from_slice(&action.slot.spill_area_offset.to_le_bytes());
        bytes.extend_from_slice(&action.store.before_instruction.0.to_le_bytes());
        bytes.extend_from_slice(&action.store.source.0.to_le_bytes());
        bytes.extend_from_slice(&action.store.source_view.0.to_le_bytes());
        bytes.extend_from_slice(&action.store.slot.0.to_le_bytes());
        bytes.extend_from_slice(&action.reload.before_instruction.0.to_le_bytes());
        bytes.extend_from_slice(&action.reload.slot.0.to_le_bytes());
        bytes.extend_from_slice(&action.reload.result.0.to_le_bytes());
        bytes.extend_from_slice(&action.reload.destination_class.0.to_le_bytes());
        length(&mut bytes, action.rewrites.len());
        for rewrite in &action.rewrites {
            bytes.extend_from_slice(&rewrite.block.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.point.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.instruction.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.operand.to_le_bytes());
            bytes.extend_from_slice(&rewrite.result.0.to_le_bytes());
        }
    }
    AbstractSpillInsertionIdentity(Sha256::digest(bytes).into())
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("abstract spill insertion length fits u64")
            .to_le_bytes(),
    );
}
