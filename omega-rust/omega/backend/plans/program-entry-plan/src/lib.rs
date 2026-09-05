#![forbid(unsafe_code)]

//! Data-only declarations that join a selected source entry to its target
//! contract and native realization. This crate owns no emitted bytes,
//! installation state, or legacy backend pipeline.

mod boundary_entry_storage;
mod diagnostic;
mod optimized_semantic_entry;
mod optimized_semantic_wrapper;
#[cfg(feature = "installed-writer")]
mod post_handoff_writer;
mod program_entry_physical;
mod root_role;
mod selected_entry;
mod service_establishment;
mod source_signature;
mod uefi_handle_protocol;

pub use boundary_entry_storage::{
    DerivedBoundaryEntryParameterStorage, DerivedBoundaryEntryStorage,
};
pub use diagnostic::ProgramStorageEntryDiagnostic;
pub use optimized_semantic_entry::*;
pub use optimized_semantic_wrapper::*;
#[cfg(feature = "installed-writer")]
pub use post_handoff_writer::*;
pub use program_entry_physical::*;
pub use root_role::ProgramStorageEntryRootRole;
pub use selected_entry::SelectedProgramStorageEntryPlan;
pub use service_establishment::ProgramEntryFusedServiceEstablishment;
pub use source_signature::{
    ProgramEntrySourceExtentFieldLayout, ProgramEntrySourceExtentFieldRole,
    ProgramEntrySourceExtentValueLayout, ProgramEntrySourceReceiverSignature,
    ProgramEntrySourceResultSignature, ProgramEntrySourceSignatureIdentity,
    ProgramEntrySourceVisibleParameterSignature, SelectedProgramEntrySourceSignature,
};
pub use uefi_handle_protocol::{
    UEFI_HANDLE_PROTOCOL_GUID_POINTER_TYPE_IDENTITY, UEFI_HANDLE_PROTOCOL_HANDLE_TYPE_IDENTITY,
    UEFI_HANDLE_PROTOCOL_INTERFACE_OUT_TYPE_IDENTITY, UEFI_HANDLE_PROTOCOL_SERVICE_IDENTITY,
    UEFI_HANDLE_PROTOCOL_STATUS_TYPE_IDENTITY, UefiHandleProtocolInvocationPlan,
    UefiHandleProtocolInvocationPlanError, UefiHandleProtocolStatus, UefiHandleProtocolStatusRow,
    plan_uefi_handle_protocol_invocation,
};
