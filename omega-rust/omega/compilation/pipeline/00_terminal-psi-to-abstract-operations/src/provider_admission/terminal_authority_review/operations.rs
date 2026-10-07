//! Exhaustive abstract-operation to terminal-authority edge classification.

use abstract_operations::AbstractOperation;
use semantic_vocabulary::{BoundaryMachineId, MachineId};

pub(super) enum AuthorityEdge {
    None,
    Internal(MachineId),
    Dynamic { interface: u32, slot: u32 },
    Boundary(BoundaryMachineId),
    CheckedPortWrite { port: u16 },
}

/// Exhaustive match: adding an abstract operation forces an explicit D45
/// decision rather than silently treating a new call/physical role as pure.
pub(super) fn authority_edge(operation: &AbstractOperation) -> AuthorityEdge {
    match operation {
        AbstractOperation::CallStructural { callee, .. } => match callee {
            abstract_operations::AbstractCallTarget::Machine(machine) => AuthorityEdge::Internal(*machine),
            abstract_operations::AbstractCallTarget::Dynamic { interface, slot, .. } => AuthorityEdge::Dynamic { interface: *interface, slot: *slot },
        },
        AbstractOperation::Call { callee, .. } => AuthorityEdge::Internal(*callee),
        AbstractOperation::BoundaryCall { boundary, .. }
        | AbstractOperation::ExternalCompletion { boundary, .. } => AuthorityEdge::Boundary(*boundary),
        AbstractOperation::PortWrite { port, .. } => {
            AuthorityEdge::CheckedPortWrite { port: *port }
        }
        // An atomic event carries no call or boundary edge; its ordering and
        // custody evidence stay on the operation itself.
        AbstractOperation::AtomicEvent { .. }
        | AbstractOperation::SelectImplementation { .. }
        | AbstractOperation::WriteOnlyPrimitiveStore { .. }
        | AbstractOperation::ByteSequenceWrite { .. }
        | AbstractOperation::StructuralByteSequenceFieldByteStore { .. }
        | AbstractOperation::ByteSequenceStore { .. }
        | AbstractOperation::StructuralByteSequenceFieldStore { .. }
        | AbstractOperation::StructuralByteViewFieldStore { .. }
        | AbstractOperation::EstablishPrimitiveLocal { .. }
        | AbstractOperation::PrimitiveLocalStore { .. }
        | AbstractOperation::PrimitiveScalarRead { .. }
        | AbstractOperation::StructuralCaseMembership { .. }
        | AbstractOperation::StructuralScalarFieldStore { .. }
        | AbstractOperation::EstablishScalarArray { .. } | AbstractOperation::EstablishStructuralArray { .. }
        | AbstractOperation::EstablishCase { .. }
        | AbstractOperation::EstablishByteSequenceLiteral { .. }
        | AbstractOperation::EstablishByteSequenceConcatenation { .. }
        | AbstractOperation::EstablishTrivialAffineLocal { .. }
        | AbstractOperation::EstablishZeroedStructural { .. }
        | AbstractOperation::EstablishStructuralLocal { .. }
        | AbstractOperation::EstablishRecord { .. }
        // Reference establishment/release are compile-time custody markers;
        // they carry no call, boundary, or physical edge of their own.
        | AbstractOperation::EstablishReference { .. }
        | AbstractOperation::ReleaseReference { .. }
        // Restoration-window extraction and repair are structural custody
        // events on the borrowed root; they carry no call, boundary, or
        // physical edge either.
        | AbstractOperation::MoveStructuralField { .. }
        | AbstractOperation::StoreStructuralField { .. }
        // A leaf copy is a read-only structural observation on a readable
        // root; it carries no call, boundary, or physical edge either.
        | AbstractOperation::StructuralLeafCopy { .. }
        | AbstractOperation::IntegerConstant { .. }
        | AbstractOperation::IeeeFloatConstant { .. }
        | AbstractOperation::IeeeFloatCompare { .. }
        | AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { .. }
        | AbstractOperation::BooleanConstant { .. }
        | AbstractOperation::ByteSequenceRead { .. }
        | AbstractOperation::ByteSequencePredicate { .. }
        | AbstractOperation::ByteSequenceEqual { .. }
        | AbstractOperation::ByteSequenceSubslice { .. }
        | AbstractOperation::ByteSequenceLength { .. }
        | AbstractOperation::EstablishElementView { .. }
        | AbstractOperation::ElementViewLength { .. }
        | AbstractOperation::ElementViewRead { .. }
        | AbstractOperation::ElementViewWrite { .. }
        | AbstractOperation::ElementViewSubslice { .. }
        | AbstractOperation::StructuralByteSequenceFieldLength { .. }
        | AbstractOperation::StructuralByteSequenceFieldRead { .. }
        | AbstractOperation::StructuralScalarFieldRead { .. }
        | AbstractOperation::BooleanNot { .. }
        | AbstractOperation::BooleanEqual { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::IntegerBitwiseNot { .. }
        | AbstractOperation::IntegerWiden { .. }
        | AbstractOperation::IntegerExactCast { .. }
        | AbstractOperation::ScalarRepresentationView { .. }
        | AbstractOperation::IntegerBitwiseAnd { .. }
        | AbstractOperation::IntegerBitwiseOr { .. }
        | AbstractOperation::IntegerBitwiseXor { .. }
        | AbstractOperation::IntegerShift { .. }
        | AbstractOperation::IntegerArithmetic { .. }
        // A Trapping primitive stops through an inline architectural trap,
        // like the `Crash` leaf; it names no call, boundary, or port.
        | AbstractOperation::TrappingInteger { .. }
        | AbstractOperation::FloatResultPolicy { .. }
        | AbstractOperation::Jump { .. }
        | AbstractOperation::Conditional { .. }
        | AbstractOperation::StructuralCase { .. }
        | AbstractOperation::Return { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::Crash { .. } => AuthorityEdge::None,
    }
}
