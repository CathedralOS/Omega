//! Pressure-recovery records for in-process allocation recovery.
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RecoveryClassificationIdentity,
    SpillChoiceIdentity,
};
pub use identity::pressure_rematerialization_identity;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    LiveRangeIdentity, LiveRangePoint, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlan, VirtualRegisterId,
};
use semantic_vocabulary::{IntegerValue, MachineId, ValueId, WorkScheduleIdentity};
use std::sync::Arc;

use crate::PressureRematerializationPolicy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureRematerializationPlan {
    pub source_selected: Arc<SelectedInstructionPlan>,
    pub spill_choices: SpillChoiceIdentity,
    pub recovery_classifications: RecoveryClassificationIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub policy: PressureRematerializationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionPressureRematerialization>,
    pub transformed_selected: Arc<SelectedInstructionPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionPressureRematerialization {
    pub machine: MachineId,
    pub action: Option<PressureRematerializationAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureRematerializationAction {
    pub block: SelectedBlockId,
    pub pressure_point: LiveRangePoint,
    pub victim: VirtualRegisterId,
    pub current_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
    pub original_materialize: SelectedInstructionId,
    pub source_value: ValueId,
    pub value: IntegerValue,
    pub rewrites: Vec<PressureRematerializationRewrite>,
    pub fresh_materialize: SelectedInstructionId,
    pub result_virtual_register: VirtualRegisterId,
    pub materialize_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PressureRematerializationRewrite {
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
}

pub(crate) mod identity {
    use crate::{PressureRematerializationPlan, PressureRematerializationPolicy};
    use selected_instructions::PressureRematerializationIdentity;
    use wire::codec_bytes::push_encoded;

    pub fn pressure_rematerialization_identity(
        plan: &PressureRematerializationPlan,
    ) -> PressureRematerializationIdentity {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"omega.terminal-pressure-rematerialization.v3\0");
        bytes.extend_from_slice(&encode_terminal_pressure_rematerialization_content(plan));
        PressureRematerializationIdentity::from_canonical_bytes(&bytes)
    }

    fn encode_terminal_pressure_rematerialization_content(
        plan: &PressureRematerializationPlan,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&plan.spill_choices.bytes());
        bytes.extend_from_slice(&plan.recovery_classifications.bytes());
        bytes.extend_from_slice(&plan.ranges.bytes());
        bytes.extend_from_slice(&plan.legality.bytes());
        bytes.extend_from_slice(&plan.register_environment.bytes());
        bytes.extend_from_slice(&plan.allocator_availability.bytes());
        bytes.extend_from_slice(&plan.optimization_unit.bytes());
        push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
        bytes.push(match plan.policy {
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => 0,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => 1,
        });
        bytes.extend_from_slice(&plan.budget.encode());
        bytes.extend_from_slice(&plan.usage.encode());
        bytes.extend_from_slice(
            &u64::try_from(plan.functions.len())
                .expect("function count fits u64")
                .to_le_bytes(),
        );
        for function in &plan.functions {
            push_encoded(&mut bytes, function.machine.get().to_le_bytes());
            match &function.action {
                None => bytes.push(0),
                Some(action) => {
                    bytes.push(1);
                    push_encoded(&mut bytes, action.block.0.to_le_bytes());
                    push_encoded(&mut bytes, action.pressure_point.0.to_le_bytes());
                    push_encoded(&mut bytes, action.victim.0.to_le_bytes());
                    push_encoded(&mut bytes, action.current_view.0.to_le_bytes());
                    push_encoded(&mut bytes, action.reclaimed_view.0.to_le_bytes());
                    push_encoded(&mut bytes, action.original_materialize.0.to_le_bytes());
                    push_encoded(&mut bytes, action.source_value.get().to_le_bytes());
                    match action.value {
                        semantic_vocabulary::IntegerValue::Unsigned(value) => {
                            push_encoded(&mut bytes, value.to_le_bytes());
                            bytes.push(0);
                        }
                        semantic_vocabulary::IntegerValue::Signed(value) => {
                            push_encoded(&mut bytes, value.to_le_bytes());
                            bytes.push(1);
                        }
                    }
                    bytes.extend_from_slice(
                        &u64::try_from(action.rewrites.len())
                            .expect("rewrite count fits u64")
                            .to_le_bytes(),
                    );
                    for rewrite in &action.rewrites {
                        push_encoded(&mut bytes, rewrite.point.0.to_le_bytes());
                        push_encoded(&mut bytes, rewrite.instruction.0.to_le_bytes());
                        push_encoded(&mut bytes, rewrite.operand.to_le_bytes());
                    }
                    push_encoded(&mut bytes, action.fresh_materialize.0.to_le_bytes());
                    push_encoded(&mut bytes, action.result_virtual_register.0.to_le_bytes());
                    register_model::encode_constraint_key(
                        &mut bytes,
                        action.materialize_constraint,
                    );
                }
            }
        }
        bytes
    }
}
