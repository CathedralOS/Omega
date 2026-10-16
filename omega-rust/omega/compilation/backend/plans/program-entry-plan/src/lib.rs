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
//! `entry_signature.rs` owns the completed declaration/layout contract. Source
//! capture belongs to provider planning; no source arena is needed here.

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;
mod entry_signature;
mod optimized_semantic_entry;
mod optimized_semantic_wrapper;
#[cfg(feature = "installed-writer")]
mod post_handoff_writer;
mod program_entry_physical;
mod selected_entry;
mod uefi;

pub(crate) use entry_signature::{ProgramEntryExtentFieldRole, ProgramEntryResultSignature};
pub use entry_signature::{
    ProgramEntryExtentValueLayout, ProgramEntryReceiverSignature, SelectedProgramEntrySignature,
};
pub(crate) use entry_signature::{
    ProgramEntrySignatureIdentity, ProgramEntryVisibleParameterSignature,
};
pub use optimized_semantic_entry::OptimizedProgramStoragePhysicalEntryDisposition;
pub use optimized_semantic_entry::{
    OptimizedProgramStorageSemanticCallingApplication,
    OptimizedProgramStorageSemanticEntryContract,
    bind_optimized_program_storage_semantic_entry_contract,
};
pub(crate) use optimized_semantic_wrapper::OptimizedProgramStorageSemanticWrapperPlan;
pub use optimized_semantic_wrapper::{
    OptimizedProgramStorageSemanticReceiverLayout,
    OptimizedProgramStorageSemanticWrapperEncodingError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    plan_optimized_program_storage_semantic_wrapper,
    select_optimized_program_storage_semantic_wrapper_encoding,
};
pub(crate) use optimized_semantic_wrapper::{
    OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperRelocationKind,
    OptimizedProgramStorageSemanticWrapperStep,
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
    MACOS_X86_64_I32_TYPE_IDENTITY, ProgramEntryPhysicalContractPackageSourceDigest,
    ProgramEntryPhysicalContractPlan, WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
    WINDOWS_X86_64_U32_TYPE_IDENTITY, exact_linux_arm64_physical_boundary_entry_plan,
    exact_linux_arm64_physical_contract_package_source_digest,
    exact_linux_x86_64_physical_boundary_entry_plan,
    exact_linux_x86_64_physical_contract_package_source_digest,
    exact_macos_arm64_physical_boundary_entry_plan,
    exact_macos_arm64_physical_contract_package_source_digest,
    exact_macos_x86_64_physical_boundary_entry_plan,
    exact_macos_x86_64_physical_contract_package_source_digest,
    exact_uefi_x64_physical_contract_package_source_digest,
    exact_windows_x86_64_physical_boundary_entry_plan,
    exact_windows_x86_64_physical_contract_package_source_digest,
};
pub use selected_entry::SelectedProgramStorageEntryPlan;

pub(crate) use selected_entry::diagnostic::ProgramStorageEntryDiagnostic;
pub use selected_entry::root_role::ProgramStorageEntryRootRole;
pub use selected_entry::service_establishment::{
    ProgramEntryFusedServiceEstablishment, ProgramEntryReceivingRequirement,
};

pub use uefi::uefi_os_handoff::uefi_os_handoff_exhaustion_requires_error;
mod source_guards;
