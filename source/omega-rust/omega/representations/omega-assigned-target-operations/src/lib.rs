#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Concrete target-operation homes and scalar-control carriers.
//!
//! Assignment plans retain target, function, structural-Unit, scalar
//! expression, control-flow, frame, and call-placement custody.

mod operation;
mod plan;
mod scalar;
mod unit;

pub use operation::{
    AssignedDynamicDescriptorParameterAbi, AssignedDynamicParameterCallMechanism, AssignedOperation,
};
pub use plan::{
    AssignedFunction, AssignedNativeCallbackArgument, AssignedOperationPlan,
    AssignedOperationPlanWithNativeCallbacks,
};
pub use scalar::{
    AssignedBooleanControl, AssignedBooleanExpression, AssignedCallArgument,
    AssignedCallDestination, AssignedConditionalBooleanArm, AssignedConditionalIntegerArm,
    AssignedIntegerControl, AssignedIntegerExpression, AssignedScalarExpression,
    AssignedScalarLocation, EntryRegisterSpill, ExpressionFrame,
};
pub use unit::{
    AssignedAggregateCopy, AssignedBoundaryResult, AssignedDynamicDescriptorArgument,
    AssignedDynamicDescriptorInstanceArgument, AssignedDynamicTraitDescriptorAbi,
    AssignedIeeeFloatFmaOperand, AssignedNormalizedForeignScalarArgument,
    AssignedRankedU32Countdown, AssignedStructuralHome, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource, AssignedUnitScalarCallArgument, AssignedUnitScalarHome,
    AssignedUnitStructuralCasePayload, AssignedUnitStructuralCaseSuccessor,
};
