/*
Installed selected-provider closure review for compiler-intrinsic,
normalized-foreign, direct-syscall, and bounded checked-physical roles.

`review_terminal_authority_closure` walks the reachable authority edges from the
program entry and expands every reachable boundary against `ReviewContext` and
`Reviewer`. When `permission_policy` is `Some`, every leaf is adjudicated
against one exact supplied row and the policy identity is bound into the
receipt; when `None`, exact mechanisms and available classifications are
retained without claiming receiver admission.
*/

/*
`physical_authority_use` exhaustively classifies `AbstractOperation` variants so
adding a new abstract operation forces an explicit physical-authority decision
rather than silently treating a new call or physical role as pure. Synchronous
call closure is owned by Terminal installation rather than this matcher.
*/

mod context;
pub(crate) mod reachability;
mod reviewer;

// Receiver admission derives constrained file-handle occurrences from the
// retained executable graph. The permission table is an independent input.
#[cfg(test)]
mod windows_handle_activations;
mod windows_handle_aliases;
mod windows_handle_arguments;
mod windows_handle_calls;
#[cfg(test)]
mod windows_handle_conditions;
mod windows_handle_contracts;
mod windows_handle_inputs;
mod windows_handle_lifetimes;
mod windows_handle_origins;
#[cfg(test)]
mod windows_handle_paths;
mod windows_handle_replay;
mod windows_handle_scalar_table;
mod windows_handle_storage;
mod windows_handle_values;

use abstract_operations::AbstractOperation;
use effects::{SelectedProviderPlanFacts, TerminalAuthorityClosureReviewReceipt};

use super::{TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy};
use crate::provider_admission::AdmittedTerminalMechanism;
use context::ReviewContext;
use reviewer::Reviewer;

pub(crate) fn review_terminal_authority_closure(
    terminal_artifact_identity: [u8; 32],
    target_profile: target::TargetProfile,
    input: &crate::AdmissionSource<'_>,
    selected: &SelectedProviderPlanFacts,
    physical_policy: &TerminalAuthorityPolicy,
    permission_policy: Option<&TerminalAuthorityPermissionPolicy>,
    mechanisms: &[AdmittedTerminalMechanism],
    settlements: &[crate::AdmittedBoundarySettlement<'_>],
    installed_candidates: &[terminal_psi::ProviderCandidateConformance],
    components: &[crate::provider_admission::NativeProviderSettlement<'_>],
    installed_interrupts: &crate::AdmittedInstalledInterruptCohort,
) -> Result<TerminalAuthorityClosureReviewReceipt, String> {
    let context = ReviewContext::new(
        target_profile,
        input,
        selected,
        physical_policy,
        permission_policy,
        mechanisms,
        settlements,
        installed_candidates,
        components,
        installed_interrupts,
    )?;
    let root_edges = context
        .graph
        .reachable_authority_edges(input.plan().entry)?;
    let mut reviewer = Reviewer::new(context, terminal_artifact_identity);
    for instruction in root_edges.checked_operations {
        reviewer.admit_program_root(instruction)?;
    }
    for boundary in root_edges.boundaries {
        reviewer.expand_boundary(boundary)?;
    }
    let (leaves, leaf_identities, physical_identity) = reviewer.into_leaves();
    TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
        terminal_artifact_identity,
        target_profile.native_target(),
        selected.identity_digest(),
        physical_identity,
        permission_policy.map(|policy| policy.identity()),
        leaves,
        &leaf_identities,
    )
    .map_err(|error| format!("terminal-authority review receipt rejected: {error:?}"))
}

pub(super) enum PhysicalAuthorityUse {
    None,
    CheckedPortWrite {
        service: semantic_vocabulary::ServiceId,
        port: u16,
        operation: semantic_vocabulary::OperationId,
    },
    CheckedRegister {
        service: Option<semantic_vocabulary::ServiceId>,
        register_operation: terminal_psi::RegisterOperation,
        operation: semantic_vocabulary::OperationId,
    },
}

pub(super) fn physical_authority_use(operation: &AbstractOperation) -> PhysicalAuthorityUse {
    match operation {
        AbstractOperation::EstablishNativeCodeLayout { .. }
        | AbstractOperation::EstablishActivationClaim { .. }
        | AbstractOperation::SplitActivationClaim { .. }
        | AbstractOperation::MergeActivationClaim { .. }
        | AbstractOperation::ReleaseActivationClaim { .. } => PhysicalAuthorityUse::None,
        AbstractOperation::CallStructural { .. }
        | AbstractOperation::Call { .. }
        | AbstractOperation::InvokeNoNormal { .. }
        | AbstractOperation::BoundaryCall { .. }
        | AbstractOperation::ExternalCompletion { .. } => PhysicalAuthorityUse::None,
        AbstractOperation::PortWrite {
            service,
            port,
            psi_operation,
            ..
        } => PhysicalAuthorityUse::CheckedPortWrite {
            service: *service,
            port: *port,
            operation: *psi_operation,
        },
        AbstractOperation::ActivateTaskRegister {
            service,
            psi_operation,
            ..
        } => PhysicalAuthorityUse::CheckedRegister {
            service: Some(*service),
            register_operation: terminal_psi::RegisterOperation::TaskRegisterActivate,
            operation: *psi_operation,
        },
        AbstractOperation::ReadTaskRegister {
            service,
            psi_operation,
            ..
        } => PhysicalAuthorityUse::CheckedRegister {
            service: Some(*service),
            register_operation: terminal_psi::RegisterOperation::TaskRegisterRead,
            operation: *psi_operation,
        },
        AbstractOperation::ReloadSegmentDescriptors {
            service,
            psi_operation,
            kind,
            ..
        } => PhysicalAuthorityUse::CheckedRegister {
            service: Some(*service),
            register_operation: terminal_psi::RegisterOperation::SegmentReload(*kind),
            operation: *psi_operation,
        },
        AbstractOperation::ReadSegmentSelector {
            service,
            psi_operation,
            register,
            ..
        } => PhysicalAuthorityUse::CheckedRegister {
            service: Some(*service),
            register_operation: terminal_psi::RegisterOperation::SegmentRead(*register),
            operation: *psi_operation,
        },
        AbstractOperation::GlobalDescriptorTable {
            service,
            operation,
            psi_operation,
            ..
        } => PhysicalAuthorityUse::CheckedRegister {
            service: Some(*service),
            register_operation: terminal_psi::RegisterOperation::GlobalDescriptorTable(*operation),
            operation: *psi_operation,
        },
        AbstractOperation::RegisterAccess {
            service,
            event,
            psi_operation,
            ..
        } => PhysicalAuthorityUse::CheckedRegister {
            service: *service,
            register_operation: event.operation(),
            operation: *psi_operation,
        },
        AbstractOperation::AtomicEvent { .. }
        | AbstractOperation::SelectImplementation { .. }
        | AbstractOperation::SelectMachine { .. }
        | AbstractOperation::WriteOnlyPrimitiveStore { .. }
        | abstract_operations::byte_sequence_write_pair_pattern!(..)
        | abstract_operations::byte_sequence_store_pair_pattern!(..)
        | AbstractOperation::StructuralByteViewFieldStore { .. }
        | AbstractOperation::EstablishPrimitiveLocal { .. }
        | AbstractOperation::PrimitiveLocalStore { .. }
        | abstract_operations::primitive_storage_observation_pattern!(..)
        | AbstractOperation::StructuralScalarFieldStore { .. }
        | AbstractOperation::EstablishScalarArray { .. }
        | abstract_operations::structural_array_or_case_pattern!(..)
        | abstract_operations::establish_byte_literal_triple_pattern!(..)
        | abstract_operations::structural_establish_triple_pattern!(..)
        | AbstractOperation::EstablishReference { .. }
        | AbstractOperation::EstablishAccessReference { .. }
        | AbstractOperation::ReleaseReference { .. }
        | abstract_operations::move_or_store_structural_field_pattern!()
        | AbstractOperation::StructuralLeafCopy { .. }
        | AbstractOperation::MoveStructuralValue { .. }
        | AbstractOperation::TransformContent { .. }
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
        | AbstractOperation::TrappingInteger { .. }
        | AbstractOperation::FloatResultPolicy { .. }
        | AbstractOperation::Jump { .. }
        | AbstractOperation::Conditional { .. }
        | AbstractOperation::StructuralCase { .. }
        | abstract_operations::return_or_return_unit_pattern!(..)
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::Crash { .. } => PhysicalAuthorityUse::None,
    }
}
