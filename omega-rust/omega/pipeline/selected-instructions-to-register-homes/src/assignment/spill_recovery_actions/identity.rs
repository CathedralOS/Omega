use optimization_unit::ValueDefinitionSite;
use selected_instructions::VirtualRegisterOrigin;
use semantic_vocabulary::{IeeeFloatFormat, IntegerCarrier, IntegerSign, ScalarType};
use sha2::{Digest, Sha256};

use crate::{
    LogicalSpillStorageClass, SpillRecoveryActionIdentity, SpillRecoveryActionPlan,
    SpillRecoveryActionPolicy,
};

pub fn spill_recovery_action_identity(
    plan: &SpillRecoveryActionPlan,
) -> SpillRecoveryActionIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.spill-recovery-actions.v1\0");
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.abstract_spill_insertion.bytes());
    bytes.extend_from_slice(&plan.worklist.bytes());
    bytes.extend_from_slice(&plan.choices.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        SpillRecoveryActionPolicy::EpochOneActiveResidentInstructionResultU64LaterFlexibleUsesV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.actions.len());
    for action in &plan.actions {
        bytes.extend_from_slice(&action.source_work_item.epoch.to_le_bytes());
        bytes.extend_from_slice(&action.source_work_item.ordinal.to_le_bytes());
        length(&mut bytes, action.function);
        bytes.extend_from_slice(&action.machine.get().to_le_bytes());
        bytes.extend_from_slice(&action.block.0.to_le_bytes());
        bytes.extend_from_slice(&action.pressure_point.0.to_le_bytes());
        bytes.extend_from_slice(&action.source_reload.0.to_le_bytes());
        bytes.extend_from_slice(&action.incoming_class.0.to_le_bytes());
        bytes.extend_from_slice(&action.victim.0.to_le_bytes());
        bytes.extend_from_slice(&action.victim_class.0.to_le_bytes());
        scalar(&mut bytes, action.victim_scalar_type);
        origin(&mut bytes, action.victim_origin);
        definition(&mut bytes, action.victim_definition_site);
        bytes.extend_from_slice(&action.current_view.0.to_le_bytes());
        bytes.extend_from_slice(&action.reclaimed_view.0.to_le_bytes());
        bytes.extend_from_slice(&action.storage.id.epoch.to_le_bytes());
        bytes.extend_from_slice(&action.storage.id.ordinal.to_le_bytes());
        bytes.push(match action.storage.class {
            LogicalSpillStorageClass::NonAddressUnsignedU64V1 => 0,
        });
        bytes.extend_from_slice(&action.store.before_source_reload.0.to_le_bytes());
        bytes.extend_from_slice(&action.store.before_instruction.0.to_le_bytes());
        bytes.extend_from_slice(&action.store.source.0.to_le_bytes());
        bytes.extend_from_slice(&action.store.storage.epoch.to_le_bytes());
        bytes.extend_from_slice(&action.store.storage.ordinal.to_le_bytes());
        bytes.extend_from_slice(&action.reload.before_instruction.0.to_le_bytes());
        bytes.extend_from_slice(&action.reload.storage.epoch.to_le_bytes());
        bytes.extend_from_slice(&action.reload.storage.ordinal.to_le_bytes());
        bytes.extend_from_slice(&action.reload.result.epoch.to_le_bytes());
        bytes.extend_from_slice(&action.reload.result.ordinal.to_le_bytes());
        bytes.extend_from_slice(&action.reload.destination_class.0.to_le_bytes());
        length(&mut bytes, action.rewrites.len());
        for rewrite in &action.rewrites {
            bytes.extend_from_slice(&rewrite.block.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.point.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.instruction.0.to_le_bytes());
            bytes.extend_from_slice(&rewrite.operand.to_le_bytes());
            bytes.extend_from_slice(&rewrite.result.epoch.to_le_bytes());
            bytes.extend_from_slice(&rewrite.result.ordinal.to_le_bytes());
        }
    }
    SpillRecoveryActionIdentity(Sha256::digest(bytes).into())
}

fn scalar(bytes: &mut Vec<u8>, value: ScalarType) {
    match value {
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                IntegerCarrier::Fixed => 0,
                IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                IeeeFloatFormat::Binary32 => 0,
                IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

fn origin(bytes: &mut Vec<u8>, value: VirtualRegisterOrigin) {
    match value {
        VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        } => {
            bytes.push(0);
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            length(bytes, parameter_index);
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
        VirtualRegisterOrigin::LegalizationTemporary {
            instruction,
            temporary,
            source_value,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&temporary.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
    }
}

fn definition(bytes: &mut Vec<u8>, value: ValueDefinitionSite) {
    match value {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("spill-recovery action length fits u64")
            .to_le_bytes(),
    );
}
