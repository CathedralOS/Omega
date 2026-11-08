/*
We define the declaration, layout, physical-arrival, and semantic-wrapper plans
that join a selected `ProgramEntry` declaration to its target contract and
native realization. We also lower and replay reusable post-handoff entry writer
fragments and UEFI OS-handoff exhaustion semantics.
*/

/*
We keep program-entry planning free of instruction encoders, object or image
bytes, and installation state so every target entry obligation is validated as
an explicit address-free plan before ISA realization encodes or replays it.
*/

/*
We consume target slot declarations, calling-convention entry plans, and
service schemas, and we expose validated entry signatures, physical arrival
plans, optimized semantic entry contracts, and wrapper encodings to native
realization and image emission.
*/

#![forbid(unsafe_code)]

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;
mod entry_signature;
mod optimized_semantic_entry;
mod optimized_semantic_wrapper;
#[cfg(feature = "installed-writer")]
mod post_handoff_writer;
mod program_entry_physical;
mod selected_entry;
mod source_guards;

pub use entry_signature::{ProgramEntryExtentFieldRole, ProgramEntryResultSignature};
pub use entry_signature::{ProgramEntryExtentValueLayout, SelectedProgramEntrySignature};
pub use entry_signature::{ProgramEntrySignatureIdentity, ProgramEntryVisibleParameterSignature};
pub use optimized_semantic_entry::OptimizedProgramStoragePhysicalEntryDisposition;
pub use optimized_semantic_entry::{
    OptimizedProgramStorageSemanticCallingApplication,
    OptimizedProgramStorageSemanticEntryContract,
    bind_optimized_program_storage_semantic_entry_contract,
};
pub use optimized_semantic_wrapper::{
    OptimizedProgramStorageSemanticWrapperEncodingError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    plan_optimized_program_storage_semantic_wrapper,
    select_optimized_program_storage_semantic_wrapper_encoding,
};
#[cfg(feature = "installed-writer")]
pub use post_handoff_writer::{
    LoweredPostHandoffWriter, LoweredPostHandoffWriterFragment, PostHandoffEntryWriterBindingError,
    PreparedPostHandoffEntryWriterInvocation, bind_post_handoff_entry_writer_invocation,
    lower_post_handoff_writer_fragment, validate_lowered_post_handoff_writer,
};
pub use program_entry_physical::{
    LINUX_ARM64_I32_TYPE_IDENTITY, LINUX_ARM64_U64_TYPE_IDENTITY,
    LINUX_X86_64_ADDRESS_TYPE_IDENTITY, LINUX_X86_64_I32_TYPE_IDENTITY,
    MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY,
    MACOS_X86_64_ADDRESS_TYPE_IDENTITY, MACOS_X86_64_I32_TYPE_IDENTITY,
    ProgramEntryPhysicalContractPackageSourceDigest, ProgramEntryPhysicalContractPlan,
    WINDOWS_X86_64_U32_TYPE_IDENTITY, exact_hosted_physical_boundary_entry_plan,
    exact_physical_contract_package_source_digest,
};
pub(crate) use selected_entry::ProgramStorageEntryDiagnostic;
pub use selected_entry::{ProgramStorageEntryRootRole, SelectedProgramStorageEntryPlan};

const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;

pub const fn uefi_os_handoff_exhaustion_requires_error(exhaustion_status: u64) -> bool {
    exhaustion_status & EFI_STATUS_ERROR_BIT != 0
}
