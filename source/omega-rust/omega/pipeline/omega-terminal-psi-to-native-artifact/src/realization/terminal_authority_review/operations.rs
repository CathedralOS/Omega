//! Exhaustive abstract-operation to terminal-authority edge classification.

use omega_abstract_operations::AbstractOperation;
use psi_core::{BoundaryMachineId, MachineId};

pub(super) enum AuthorityEdge {
    None,
    Internal(MachineId),
    Boundary(BoundaryMachineId),
    UnsupportedCheckedPhysical,
}

/// Exhaustive match: adding an abstract operation forces an explicit D45
/// decision rather than silently treating a new call/physical role as pure.
pub(super) fn authority_edge(operation: &AbstractOperation) -> AuthorityEdge {
    match operation {
        AbstractOperation::CallUnit { callee, .. }
        | AbstractOperation::CallStructuralScalar { callee, .. }
        | AbstractOperation::CallStructural { callee, .. }
        | AbstractOperation::Call { callee, .. } => AuthorityEdge::Internal(*callee),
        AbstractOperation::CallDynamicScalar {
            dynamic_dispatch, ..
        } => AuthorityEdge::Internal(dynamic_dispatch.dispatch.realization),
        AbstractOperation::CallDynamicParameterScalar { .. } => {
            AuthorityEdge::UnsupportedCheckedPhysical
        }
        AbstractOperation::CallStructuralScalarWithDynamicArguments { .. } => {
            AuthorityEdge::UnsupportedCheckedPhysical
        }
        AbstractOperation::BoundaryCall { boundary, .. } => AuthorityEdge::Boundary(*boundary),
        AbstractOperation::PortWrite { .. } => AuthorityEdge::UnsupportedCheckedPhysical,
        AbstractOperation::WriteOnlyPrimitiveStore { .. }
        | AbstractOperation::StructuralScalarFieldStore { .. }
        | AbstractOperation::EstablishPayloadlessCase { .. }
        | AbstractOperation::EstablishByteSequenceLiteral { .. }
        | AbstractOperation::EstablishTrivialAffineLocal { .. }
        | AbstractOperation::IntegerConstant { .. }
        | AbstractOperation::IeeeFloatConstant { .. }
        | AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { .. }
        | AbstractOperation::BooleanConstant { .. }
        | AbstractOperation::BooleanStructuralField { .. }
        | AbstractOperation::IntegerStructuralField { .. }
        | AbstractOperation::BooleanNot { .. }
        | AbstractOperation::BooleanEqual { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::IntegerBitwiseNot { .. }
        | AbstractOperation::IntegerWiden { .. }
        | AbstractOperation::IntegerExactCast { .. }
        | AbstractOperation::IntegerBitwiseAnd { .. }
        | AbstractOperation::IntegerBitwiseOr { .. }
        | AbstractOperation::IntegerBitwiseXor { .. }
        | AbstractOperation::WrappingIntegerShiftLeft { .. }
        | AbstractOperation::WrappingIntegerShiftRight { .. }
        | AbstractOperation::ExactIntegerShiftLeft { .. }
        | AbstractOperation::ExactIntegerShiftRight { .. }
        | AbstractOperation::WrappingIntegerAdd { .. }
        | AbstractOperation::ExactIntegerAdd { .. }
        | AbstractOperation::SaturatingIntegerAdd { .. }
        | AbstractOperation::WrappingIntegerSubtract { .. }
        | AbstractOperation::ExactIntegerSubtract { .. }
        | AbstractOperation::SaturatingIntegerSubtract { .. }
        | AbstractOperation::WrappingIntegerMultiply { .. }
        | AbstractOperation::ExactIntegerMultiply { .. }
        | AbstractOperation::ExactIntegerDivide { .. }
        | AbstractOperation::ExactIntegerRemainder { .. }
        | AbstractOperation::WrappingIntegerDivide { .. }
        | AbstractOperation::WrappingIntegerRemainder { .. }
        | AbstractOperation::SaturatingIntegerDivide { .. }
        | AbstractOperation::SaturatingIntegerRemainder { .. }
        | AbstractOperation::SaturatingIntegerMultiply { .. }
        | AbstractOperation::Jump { .. }
        | AbstractOperation::Conditional { .. }
        | AbstractOperation::Return { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::Crash { .. } => AuthorityEdge::None,
    }
}
