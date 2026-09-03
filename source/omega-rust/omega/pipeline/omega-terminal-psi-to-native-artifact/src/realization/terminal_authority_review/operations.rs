//! Exhaustive abstract-operation to terminal-authority edge classification.

use omega_abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractOperation, AbstractParameterDynamicDispatch,
};
use psi_core::{BoundaryMachineId, MachineId};

pub(super) enum AuthorityEdge<'operation> {
    None,
    Internal(MachineId),
    InternalWithDynamicArguments {
        callee: MachineId,
        arguments: &'operation [AbstractDynamicDescriptorArgument],
    },
    DynamicParameterDispatch(&'operation AbstractParameterDynamicDispatch),
    Boundary(BoundaryMachineId),
    CheckedPortWrite {
        service: psi_core::ServiceId,
        port: u16,
    },
}

/// Exhaustive match: adding an abstract operation forces an explicit D45
/// decision rather than silently treating a new call/physical role as pure.
pub(super) fn authority_edge(operation: &AbstractOperation) -> AuthorityEdge<'_> {
    match operation {
        AbstractOperation::CallUnit { callee, .. }
        | AbstractOperation::CallStructuralScalar { callee, .. }
        | AbstractOperation::CallStructural { callee, .. }
        | AbstractOperation::Call { callee, .. } => AuthorityEdge::Internal(*callee),
        AbstractOperation::CallDynamicScalar {
            dynamic_dispatch, ..
        }
        | AbstractOperation::CallDynamicUnit {
            dynamic_dispatch, ..
        } => AuthorityEdge::Internal(dynamic_dispatch.dispatch.realization),
        AbstractOperation::CallStoredDynamicScalar {
            dynamic_dispatch, ..
        } => AuthorityEdge::Internal(dynamic_dispatch.dispatch.realization),
        AbstractOperation::CallDynamicParameterScalar {
            dynamic_dispatch, ..
        }
        | AbstractOperation::CallDynamicParameterUnit {
            dynamic_dispatch, ..
        } => AuthorityEdge::DynamicParameterDispatch(dynamic_dispatch),
        AbstractOperation::CallStructuralScalarWithDynamicArguments {
            callee,
            dynamic_arguments,
            ..
        }
        | AbstractOperation::CallUnitWithDynamicArguments {
            callee,
            dynamic_arguments,
            ..
        } => AuthorityEdge::InternalWithDynamicArguments {
            callee: *callee,
            arguments: dynamic_arguments,
        },
        AbstractOperation::BoundaryCall { boundary, .. } => AuthorityEdge::Boundary(*boundary),
        AbstractOperation::PortWrite { service, port, .. } => AuthorityEdge::CheckedPortWrite {
            service: *service,
            port: *port,
        },
        AbstractOperation::DynamicDescriptorParameter { .. }
        | AbstractOperation::StoreDynamicDescriptor { .. }
        | AbstractOperation::WriteOnlyPrimitiveStore { .. }
        | AbstractOperation::StructuralScalarFieldStore { .. }
        | AbstractOperation::EstablishPayloadlessCase { .. }
        | AbstractOperation::EstablishByteSequenceLiteral { .. }
        | AbstractOperation::EstablishTrivialAffineLocal { .. }
        | AbstractOperation::EstablishAffineScalarRecord { .. }
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
