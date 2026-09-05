pub(crate) use std::collections::BTreeMap;

pub(crate) use assigned_target_operations::{
    AssignedAggregateCopy, AssignedBooleanControl, AssignedBooleanExpression,
    AssignedBoundaryResult, AssignedCallArgument, AssignedCallDestination,
    AssignedConditionalBooleanArm, AssignedConditionalIntegerArm,
    AssignedDynamicDescriptorArgument, AssignedDynamicDescriptorInstanceArgument,
    AssignedDynamicDescriptorParameterAbi, AssignedDynamicParameterCallMechanism,
    AssignedDynamicTraitDescriptorAbi, AssignedFunction, AssignedIeeeFloatFmaOperand,
    AssignedIntegerControl, AssignedIntegerExpression, AssignedNormalizedForeignScalarArgument,
    AssignedOperation, AssignedOperationPlan, AssignedRankedU32Countdown, AssignedScalarExpression,
    AssignedScalarLocation, AssignedStructuralHome, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource, AssignedUnitScalarCallArgument, AssignedUnitScalarHome,
    EntryRegisterSpill, ExpressionFrame,
};
pub(crate) use calling_conventions::{
    CallSignature, CallingPolicy, ValueClass, ValueLocation, ValueShape, evaluate_call_plan,
};
pub(crate) use semantic_vocabulary::{
    EdgeId, IntegerType, MachineId, OperationId, PlaceId, ValueId,
};
pub(crate) use target::{Architecture, NativeTarget};
pub(crate) use target_operations::{
    AbstractDynamicDescriptorSource, MachineRegister, ScalarParameterLocation,
    TargetBooleanControl, TargetBooleanExpression, TargetCallArgument,
    TargetDynamicDescriptorArgument, TargetFunction, TargetIeeeFloatFmaOperand,
    TargetIntegerControl, TargetIntegerExpression, TargetOperation, TargetOperationPlan,
    TargetRankedU32Countdown, TargetScalarExpression, TargetUnitOperation,
    TargetUnitScalarArgumentSource,
};

pub(crate) use crate::AssignmentError;
