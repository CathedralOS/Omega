//! Pressure-recovery records for in-process allocation recovery.
use crate::AllocatorAvailabilityIdentity;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    LiveRangePoint, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    VirtualRegisterId,
};
use semantic_vocabulary::{IntegerValue, MachineId, ValueId, WorkScheduleIdentity};
use std::sync::Arc;

use crate::PressureRematerializationPolicy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureRematerializationPlan {
    pub source_selected: Arc<SelectedInstructionPlan>,
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
