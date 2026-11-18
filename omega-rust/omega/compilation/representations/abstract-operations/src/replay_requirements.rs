//! Classify the source obligations whose current physical evidence must survive.

use crate::{AbstractOperation, AbstractStructuralCallResult};

pub fn requires_graph_storage_replay<'a>(
    operations: impl IntoIterator<Item = &'a AbstractOperation>,
) -> bool {
    operations.into_iter().any(|operation| {
        if operation.cleanup_rosters().any(|(_, actions)| {
            actions.iter().any(|action| {
                matches!(
                    action,
                    terminal_psi::TerminalAffineCleanupAction::InvokeOwned(_)
                )
            })
        }) {
            return true;
        }
        if let AbstractOperation::CallStructural {
            structural_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } = operation
            && structural_arguments
                .iter()
                .any(|argument| argument.access == terminal_psi::StructuralAccess::Owned)
        {
            return true;
        }
        matches!(
            operation,
            AbstractOperation::SelectImplementation { .. }
                | AbstractOperation::InvokeNoNormal { .. }
                | AbstractOperation::ExternalCompletion { .. }
                | AbstractOperation::CallStructural {
                    callee: crate::AbstractCallTarget::Dynamic { .. },
                    ..
                }
                | AbstractOperation::AtomicEvent { .. }
                | AbstractOperation::RegisterAccess { .. }
                | AbstractOperation::ReadSegmentSelector { .. }
                | AbstractOperation::ReadTaskRegister { .. }
                | AbstractOperation::ReloadSegmentDescriptors { .. }
                | AbstractOperation::ActivateTaskRegister { .. }
                | AbstractOperation::GlobalDescriptorTable { .. }
                | AbstractOperation::EstablishByteSequenceConcatenation { .. }
                | AbstractOperation::EstablishPrimitiveLocal { .. }
                | crate::establish_record_or_case_pattern!(..)
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishStructuralArray { .. }
                | AbstractOperation::EstablishZeroedStructural { .. }
                | AbstractOperation::EstablishStructuralLocal { .. }
                | AbstractOperation::EstablishNativeCodeLayout { .. }
                | AbstractOperation::EstablishActivationClaim { .. }
                | AbstractOperation::SplitActivationClaim { .. }
                | AbstractOperation::MergeActivationClaim { .. }
                | AbstractOperation::ReleaseActivationClaim { .. }
                | AbstractOperation::CallStructural {
                    result: AbstractStructuralCallResult::Structural { .. },
                    ..
                }
                | AbstractOperation::ReturnStructural { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
                | AbstractOperation::StorageAddress { .. }
                | AbstractOperation::PrimitiveScalarRead { .. }
                | AbstractOperation::WriteOnlyPrimitiveStore { .. }
                | AbstractOperation::StructuralScalarFieldRead { .. }
                | AbstractOperation::StructuralScalarFieldStore { .. }
                | crate::byte_len_or_field_read_pattern!(..)
                | crate::byte_sequence_store_pair_pattern!(..)
                | AbstractOperation::StructuralByteViewFieldStore { .. }
                | AbstractOperation::StructuralByteSequenceFieldByteStore { .. }
                | AbstractOperation::StructuralCaseMembership { .. }
                | AbstractOperation::StructuralLeafCopy { .. }
                | AbstractOperation::MoveStructuralValue { .. }
                | AbstractOperation::DecomposeStructuralValue { .. }
                | AbstractOperation::TransformContent { .. }
                | crate::move_or_store_structural_field_pattern!()
        )
    })
}
