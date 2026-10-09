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
        | abstract_operations::byte_sequence_write_pair_pattern!(..)
        | abstract_operations::byte_sequence_store_pair_pattern!( .. )
        | AbstractOperation::StructuralByteViewFieldStore { .. }
        | AbstractOperation::EstablishPrimitiveLocal { .. }
        | AbstractOperation::PrimitiveLocalStore { .. }
        | abstract_operations::scalar_read_or_membership_pattern!(..)
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
        | abstract_operations::move_or_store_structural_field_pattern!()
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
        | abstract_operations::element_view_read_write_pair_pattern!(..)
        | AbstractOperation::ElementViewSubslice { .. }
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | AbstractOperation::StructuralScalarFieldRead { .. }
        | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
        | abstract_operations::integer_equal_less_pair_pattern!(..)
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::IntegerBitwiseNot { .. }
        | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
        | AbstractOperation::ScalarRepresentationView { .. }
        | AbstractOperation::IntegerBitwiseAnd { .. }
        | abstract_operations::bitwise_or_xor_pair_pattern!(..)
        | AbstractOperation::IntegerShift { .. }
        | AbstractOperation::IntegerArithmetic { .. }
        // A Trapping primitive stops through an inline architectural trap,
        // like the `Crash` leaf; it names no call, boundary, or port.
        | AbstractOperation::TrappingInteger { .. }
        | AbstractOperation::FloatResultPolicy { .. }
        | AbstractOperation::Jump { .. }
        | AbstractOperation::Conditional { .. }
        | AbstractOperation::StructuralCase { .. }
        | abstract_operations::return_or_return_unit_pattern!(..)
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::Crash { .. } => AuthorityEdge::None,
    }
}
