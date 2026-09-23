#![forbid(unsafe_code)]

//! Declarations that join a selected source entry to its target contract and
//! native realization. This crate owns no instruction encoder, object or image
//! bytes, installation state, or legacy backend pipeline: where a plan names a
//! target obligation, it projects the plan onto the ISA owner's request and
//! replays what that owner encodes.
//!
//! Start at `selected_entry.rs`: it names the entry and owns its root role,
//! diagnostic, boundary storage and service establishment. Around it,
//! `program_entry_physical` its physical contract, `optimized_semantic_entry`
//! and `optimized_semantic_wrapper` the optimized wrappers (the latter with
//! its x86-64 template selection under `optimized_semantic_wrapper/encoding`),
//! and the optional `post_handoff_writer` the plan a provider replays after
//! handoff. `uefi` holds the two planning-only UEFI invocation plans.

mod optimized_semantic_entry;
mod optimized_semantic_wrapper;
#[cfg(feature = "installed-writer")]
mod post_handoff_writer;
mod program_entry_physical;
mod selected_entry;
mod source_signature;
mod uefi;

pub use optimized_semantic_entry::{
    OptimizedProgramStoragePhysicalEntryDisposition,
    OptimizedProgramStorageSemanticCallingApplication,
    OptimizedProgramStorageSemanticEntryContract, OptimizedProgramStorageSemanticRoot,
    bind_optimized_program_storage_semantic_entry_contract,
};
pub use optimized_semantic_wrapper::{
    OptimizedProgramStorageSemanticReceiverLayout, OptimizedProgramStorageSemanticReceiverStorage,
    OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperEncodingError,
    OptimizedProgramStorageSemanticWrapperPlan,
    OptimizedProgramStorageSemanticWrapperRelocationKind,
    OptimizedProgramStorageSemanticWrapperRelocationRequirement,
    OptimizedProgramStorageSemanticWrapperStep,
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    plan_optimized_program_storage_semantic_wrapper,
    select_optimized_program_storage_semantic_wrapper_encoding,
    validate_optimized_program_storage_semantic_wrapper,
    validate_optimized_program_storage_semantic_wrapper_encoding,
};
#[cfg(feature = "installed-writer")]
pub use post_handoff_writer::{
    LoweredPostHandoffWriter, LoweredPostHandoffWriterFragment, PostHandoffEntryWriterBindingError,
    PreparedPostHandoffEntryWriterInvocation, bind_post_handoff_entry_writer_invocation,
    lower_post_handoff_writer_fragment, validate_lowered_post_handoff_writer,
};
pub use program_entry_physical::{
    LINUX_ARM64_I32_TYPE_IDENTITY, LINUX_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
    LINUX_ARM64_U64_TYPE_IDENTITY, LINUX_X86_64_ADDRESS_TYPE_IDENTITY,
    LINUX_X86_64_I32_TYPE_IDENTITY, LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
    MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY,
    MACOS_ARM64_PHYSICAL_REQUIREMENT_IDENTITY, MACOS_X86_64_ADDRESS_TYPE_IDENTITY,
    MACOS_X86_64_I32_TYPE_IDENTITY, MACOS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
    ProgramEntryPhysicalContractPackageSourceDigest, ProgramEntryPhysicalContractPlan,
    UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY, UEFI_X64_PHYSICAL_CALLING_PLAN_COMMITMENT,
    UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
    UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
    WINDOWS_X86_64_U32_TYPE_IDENTITY, exact_linux_arm64_physical_boundary_entry_plan,
    exact_linux_arm64_physical_contract_package_source_digest,
    exact_linux_x86_64_physical_boundary_entry_plan,
    exact_linux_x86_64_physical_contract_package_source_digest,
    exact_macos_arm64_physical_boundary_entry_plan,
    exact_macos_arm64_physical_contract_package_source_digest,
    exact_macos_x86_64_physical_boundary_entry_plan,
    exact_macos_x86_64_physical_contract_package_source_digest,
    exact_uefi_x64_physical_boundary_entry_plan,
    exact_uefi_x64_physical_contract_package_source_digest,
    exact_windows_x86_64_physical_boundary_entry_plan,
    exact_windows_x86_64_physical_contract_package_source_digest,
    replayed_uefi_x64_physical_calling_plan,
};
pub use selected_entry::SelectedProgramStorageEntryPlan;
pub use selected_entry::boundary_entry_storage::{
    DerivedBoundaryEntryParameterStorage, DerivedBoundaryEntryStorage,
};
pub use selected_entry::diagnostic::ProgramStorageEntryDiagnostic;
pub use selected_entry::root_role::ProgramStorageEntryRootRole;
pub use selected_entry::service_establishment::ProgramEntryFusedServiceEstablishment;
pub use source_signature::{
    ProgramEntrySourceExtentFieldLayout, ProgramEntrySourceExtentFieldRole,
    ProgramEntrySourceExtentValueLayout, ProgramEntrySourceReceiverSignature,
    ProgramEntrySourceResultSignature, ProgramEntrySourceSignatureIdentity,
    ProgramEntrySourceVisibleParameterSignature, SelectedProgramEntrySourceSignature,
};
pub use uefi::uefi_handle_protocol::{
    UEFI_HANDLE_PROTOCOL_GUID_POINTER_TYPE_IDENTITY, UEFI_HANDLE_PROTOCOL_HANDLE_TYPE_IDENTITY,
    UEFI_HANDLE_PROTOCOL_INTERFACE_OUT_TYPE_IDENTITY, UEFI_HANDLE_PROTOCOL_SERVICE_IDENTITY,
    UEFI_HANDLE_PROTOCOL_STATUS_TYPE_IDENTITY, UefiHandleProtocolInvocationPlan,
    UefiHandleProtocolInvocationPlanError, UefiHandleProtocolStatus, UefiHandleProtocolStatusRow,
    plan_uefi_handle_protocol_invocation,
};
pub use uefi::uefi_os_handoff::{
    UEFI_EXIT_BOOT_SERVICES_SERVICE_IDENTITY, UEFI_GET_MEMORY_MAP_SERVICE_IDENTITY,
    UEFI_OS_HANDOFF_DESCRIPTOR_SIZE_TYPE_IDENTITY,
    UEFI_OS_HANDOFF_DESCRIPTOR_VERSION_TYPE_IDENTITY, UEFI_OS_HANDOFF_IMAGE_HANDLE_TYPE_IDENTITY,
    UEFI_OS_HANDOFF_MAP_KEY_POINTER_TYPE_IDENTITY, UEFI_OS_HANDOFF_MAP_KEY_TYPE_IDENTITY,
    UEFI_OS_HANDOFF_MEMORY_MAP_SIZE_TYPE_IDENTITY, UEFI_OS_HANDOFF_MEMORY_MAP_TYPE_IDENTITY,
    UEFI_OS_HANDOFF_STATUS_TYPE_IDENTITY, UefiOsHandoffInvocationPlan,
    UefiOsHandoffInvocationPlanError, UefiOsHandoffLegPlan, UefiOsHandoffStatusRole,
    UefiOsHandoffStatusRow, plan_uefi_os_handoff_invocation,
    uefi_os_handoff_exhaustion_requires_error,
};
