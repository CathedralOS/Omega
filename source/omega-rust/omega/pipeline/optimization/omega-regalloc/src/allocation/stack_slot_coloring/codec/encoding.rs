use super::super::*;
use super::{MAGIC, VERSION};
use crate::LogicalSpillStorageClass;

pub(super) fn encode(plan: &StackSlotColoringPlan) -> Vec<u8> {
    let content = encode_content(plan);
    let identity = super::super::stack_slot_coloring_identity(plan);
    let mut bytes = Vec::with_capacity(44 + content.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&identity.bytes());
    bytes.extend_from_slice(&content);
    bytes
}

pub(in crate::allocation::stack_slot_coloring) fn encode_content(
    plan: &StackSlotColoringPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.logical_spill_operations.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.extend_from_slice(&function.spill_area_bytes.to_le_bytes());
        encode_len(&mut bytes, function.assignments.len());
        for assignment in &function.assignments {
            bytes.extend_from_slice(&assignment.storage.0.to_le_bytes());
            bytes.push(match assignment.class {
                LogicalSpillStorageClass::NonAddressUnsignedU64V1 => 0,
            });
            bytes.extend_from_slice(&assignment.block.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.live_from.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.live_through.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.size_bytes.to_le_bytes());
            bytes.extend_from_slice(&assignment.alignment_bytes.to_le_bytes());
            bytes.extend_from_slice(&assignment.spill_area_offset.to_le_bytes());
        }
    }
    bytes
}

fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(
        &u64::try_from(length)
            .expect("stack-slot-coloring canonical length fits u64")
            .to_le_bytes(),
    );
}
