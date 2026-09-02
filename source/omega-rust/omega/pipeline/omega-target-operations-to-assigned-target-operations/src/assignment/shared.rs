pub(crate) use std::collections::BTreeMap;

pub(crate) use omega_assigned_target_operations::{
    AssignedAggregateCopy, AssignedBooleanControl, AssignedBooleanExpression, AssignedCallArgument,
    AssignedCallDestination, AssignedConditionalBooleanArm, AssignedConditionalIntegerArm,
    AssignedDynamicDescriptorParameterAbi, AssignedDynamicParameterCallMechanism,
    AssignedDynamicTraitDescriptorAbi, AssignedFunction, AssignedIeeeFloatFmaOperand,
    AssignedIntegerControl, AssignedIntegerExpression, AssignedNormalizedForeignScalarArgument,
    AssignedOperation, AssignedOperationPlan, AssignedRankedU32Countdown, AssignedScalarExpression,
    AssignedScalarLocation, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource, AssignedUnitScalarCallArgument, AssignedUnitScalarHome,
    EntryRegisterSpill, ExpressionFrame,
};
pub(crate) use omega_calling_conventions::{
    evaluate_call_plan, CallSignature, CallingPolicy, ValueClass, ValueLocation, ValueShape,
};
pub(crate) use omega_target::{Architecture, NativeTarget};
pub(crate) use omega_target_operations::{
    MachineRegister, ScalarParameterLocation, TargetBooleanControl, TargetBooleanExpression,
    TargetCallArgument, TargetFunction, TargetIeeeFloatFmaOperand, TargetIntegerControl,
    TargetIntegerExpression, TargetOperation, TargetOperationPlan, TargetRankedU32Countdown,
    TargetScalarExpression, TargetUnitOperation, TargetUnitScalarArgumentSource,
};
pub(crate) use psi_core::{EdgeId, IntegerType, MachineId, OperationId, ValueId};

pub(crate) use crate::AssignmentError;
