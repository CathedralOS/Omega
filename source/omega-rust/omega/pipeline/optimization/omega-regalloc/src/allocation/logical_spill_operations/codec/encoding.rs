use super::super::*;
use super::values::{encode_definition_site, encode_len, encode_origin, encode_scalar_type};
use super::{MAGIC, VERSION};

pub(super) fn encode(plan: &LogicalSpillOperationPlan) -> Vec<u8> {
    let content = encode_content(plan);
    let identity = super::super::logical_spill_operation_identity(plan);
    let mut bytes = Vec::with_capacity(44 + content.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&identity.bytes());
    bytes.extend_from_slice(&content);
    bytes
}

pub(in crate::allocation::logical_spill_operations) fn encode_content(
    plan: &LogicalSpillOperationPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.spill_choices.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.push(u8::from(function.action.is_some()));
        let Some(action) = &function.action else {
            continue;
        };
        bytes.extend_from_slice(&action.block.0.to_le_bytes());
        bytes.extend_from_slice(&action.pressure_point.0.to_le_bytes());
        bytes.extend_from_slice(&action.incoming.0.to_le_bytes());
        bytes.extend_from_slice(&action.incoming_class.0.to_le_bytes());
        bytes.extend_from_slice(&action.victim.0.to_le_bytes());
        bytes.extend_from_slice(&action.victim_class.0.to_le_bytes());
        encode_scalar_type(&mut bytes, action.victim_scalar_type);
        encode_origin(&mut bytes, action.victim_origin);
        encode_definition_site(&mut bytes, action.victim_definition_site);
        bytes.extend_from_slice(&action.current_view.0.to_le_bytes());
        bytes.extend_from_slice(&action.reclaimed_view.0.to_le_bytes());
        bytes.extend_from_slice(&action.storage.id.0.to_le_bytes());
        bytes.push(match action.storage.class {
            LogicalSpillStorageClass::NonAddressUnsignedU64V1 => 0,
        });
        bytes.extend_from_slice(&action.store.before_instruction.0.to_le_bytes());
        bytes.extend_from_slice(&action.store.source.0.to_le_bytes());
        bytes.extend_from_slice(&action.store.storage.0.to_le_bytes());
        bytes.extend_from_slice(&action.reload.before_instruction.0.to_le_bytes());
        bytes.extend_from_slice(&action.reload.storage.0.to_le_bytes());
        bytes.extend_from_slice(&action.reload.result.0.to_le_bytes());
        encode_len(&mut bytes, action.rewrites.len());
        for rewrite in &action.rewrites {
            bytes.extend_from_slice(&rewrite.block.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.point.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.instruction.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.operand.to_le_bytes());
            bytes.extend_from_slice(&rewrite.result.0.to_le_bytes());
        }
    }
    bytes
}
