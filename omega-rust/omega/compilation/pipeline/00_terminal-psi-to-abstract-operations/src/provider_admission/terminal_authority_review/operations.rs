//! Exhaustive abstract-operation to terminal-authority edge classification.

use abstract_operations::AbstractOperation;
use semantic_vocabulary::{BoundaryMachineId, MachineId};

pub(super) enum AuthorityEdge {
    None,
    Internal(MachineId),
    Dynamic {
        interface: u32,
        slot: u32,
    },
    Boundary(BoundaryMachineId),
    CheckedPortWrite {
        service: semantic_vocabulary::ServiceId,
        port: u16,
        operation: semantic_vocabulary::OperationId,
    },
    CheckedRegister {
        service: semantic_vocabulary::ServiceId,
        register_operation: terminal_psi::RegisterOperation,
        operation: semantic_vocabulary::OperationId,
    },
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
        AbstractOperation::PortWrite { service, port, psi_operation, .. } => {
            AuthorityEdge::CheckedPortWrite { service: *service, port: *port, operation: *psi_operation }
        }
        AbstractOperation::RegisterAccess { service, event, psi_operation, .. } => {
            AuthorityEdge::CheckedRegister {
                service: *service,
                register_operation: event.operation(),
                operation: *psi_operation,
            }
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
        | AbstractOperation::EstablishScalarArray { .. } | abstract_operations::structural_array_or_case_pattern!(..)
        | abstract_operations::establish_byte_literal_triple_pattern!(..)
        | abstract_operations::structural_establish_triple_pattern!(..)
        // Reference establishment/release are compile-time custody markers;
        // they carry no call, boundary, or physical edge of their own.
        | AbstractOperation::EstablishReference { .. }
        | AbstractOperation::EstablishPlacedReference { .. }
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
        | abstract_operations::byte_read_predicate_equal_pattern!(..)
        | abstract_operations::element_view_or_byte_subslice_pattern!(..)
        | abstract_operations::byte_view_length_triple_pattern!(..)
        | abstract_operations::element_view_read_write_pair_pattern!(..)
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | AbstractOperation::StructuralScalarFieldRead { .. }
        | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
        | abstract_operations::integer_equal_less_pair_pattern!(..)
        | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
        | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
        | abstract_operations::bitwise_and_or_view_pair_pattern!(..)
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
