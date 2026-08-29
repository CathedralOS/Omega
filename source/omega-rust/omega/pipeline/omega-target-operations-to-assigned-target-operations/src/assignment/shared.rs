pub(crate) use std::collections::BTreeMap;

pub(crate) use omega_assigned_target_operations::{
    AssignedAggregateCopy, AssignedBooleanControl, AssignedBooleanExpression, AssignedCallArgument,
    AssignedCallDestination, AssignedConditionalBooleanArm, AssignedConditionalIntegerArm,
    AssignedFunction, AssignedIntegerControl, AssignedIntegerExpression, AssignedOperation,
    AssignedOperationPlan, AssignedScalarExpression, AssignedScalarLocation, AssignedUnitBody,
    AssignedUnitOperation, EntryRegisterSpill, ExpressionFrame,
};
pub(crate) use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueClass, ValueLocation, evaluate_call_plan,
};
pub(crate) use omega_target::{Architecture, NativeTarget};
pub(crate) use omega_target_operations::{
    MachineRegister, ScalarParameterLocation, TargetBooleanControl, TargetBooleanExpression,
    TargetCallArgument, TargetFunction, TargetIntegerControl, TargetIntegerExpression,
    TargetOperation, TargetOperationPlan, TargetScalarExpression, TargetUnitOperation,
};
pub(crate) use psi_core::{EdgeId, MachineId, OperationId, ValueId};

pub(crate) use crate::AssignmentError;
