use super::{
    boundary, cleanup, dynamic_parameter, ranked_countdown, scalar, scalar_store, structural,
    structural_parameter, unit,
};
use crate::assignment::shared::*;

/// Exhaustive target-carrier classification. Each arm names the lower semantic
/// owner; adding a carrier cannot silently fall into a generic assignment path.
pub(super) fn assign_operation(
    function: &TargetFunction,
    target: NativeTarget,
    native_callbacks: &[omega_target_operations::TargetNativeCallbackArgument],
) -> Result<AssignedOperation, AssignmentError> {
    match &function.operation {
        TargetOperation::RankedU32Countdown(countdown) => {
            ranked_countdown::assign(countdown, target)
        }
        operation @ TargetOperation::ReturnStructuralScalarCall { .. } => {
            structural::scalar_call_result::assign(function.machine, operation, target)
        }
        operation @ (TargetOperation::ReturnDynamicParameterScalarCall { .. }
        | TargetOperation::DynamicParameterUnitCall { .. }) => {
            dynamic_parameter::assign(function, operation, target)
        }
        operation @ TargetOperation::ReturnStructuralCall { .. } => {
            structural::direct_call_result::assign(function.machine, operation, target)
        }
        operation @ (TargetOperation::ScalarReturnWithCleanup { .. }
        | TargetOperation::BooleanControlWithCleanup { .. }) => {
            cleanup::assign(function, operation, target)
        }
        operation @ TargetOperation::ScalarReturnAfterStructuralScalarFieldStore { .. } => {
            scalar_store::assign(function, operation, target)
        }
        operation @ (TargetOperation::ReturnBoundaryPortReadU8 { .. }
        | TargetOperation::ExitProcessI32 { .. }) => boundary::assign(function, operation, target),
        operation @ TargetOperation::UnitBody(_) => {
            unit::assign(function, operation, target, native_callbacks)
        }
        operation @ TargetOperation::ReturnStructuralParameter { .. } => {
            structural_parameter::assign(operation, target)
        }
        operation @ (TargetOperation::Crash { .. }
        | TargetOperation::ReturnIntegerImmediate { .. }
        | TargetOperation::ReturnBooleanImmediate { .. }
        | TargetOperation::ReturnIntegerParameter { .. }
        | TargetOperation::ReturnBooleanParameter { .. }
        | TargetOperation::ReturnBooleanNotParameter { .. }
        | TargetOperation::ReturnBooleanSharedConvergence { .. }
        | TargetOperation::ReturnBooleanExpression { .. }
        | TargetOperation::ReturnIntegerExpression { .. }
        | TargetOperation::ReturnIntegerConditionalControl { .. }
        | TargetOperation::ReturnIntegerExpressionConditionalControl { .. }
        | TargetOperation::ReturnBooleanConditionalControl { .. }
        | TargetOperation::ReturnBooleanExpressionConditionalControl { .. }) => {
            scalar::assign(operation, target.architecture)
        }
    }
}
