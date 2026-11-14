/*
We define the declaration, layout, and physical-arrival plans
that join a selected `ProgramEntry` declaration to its target contract and
native realization, including UEFI OS-handoff exhaustion semantics.
*/

/*
We keep program-entry planning free of instruction encoders, object or image
bytes, and installation state so every target entry obligation is validated as
an explicit address-free plan before ISA realization encodes or replays it.
*/

/*
We consume target slot declarations, calling-convention entry plans, and
service schemas, and we expose validated entry signatures and physical arrival
plans to native realization and image emission.
*/

#![forbid(unsafe_code)]

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;
mod entry_signature;
mod program_entry_physical;
mod selected_entry;
mod source_guards;

pub use entry_signature::{ProgramEntryExtentFieldRole, ProgramEntryResultSignature};
pub use entry_signature::{ProgramEntryExtentValueLayout, SelectedProgramEntrySignature};
pub use entry_signature::{ProgramEntrySignatureIdentity, ProgramEntryVisibleParameterSignature};
pub use program_entry_physical::{
    LINUX_ARM64_I32_TYPE_IDENTITY, LINUX_ARM64_U64_TYPE_IDENTITY,
    LINUX_X86_64_ADDRESS_TYPE_IDENTITY, LINUX_X86_64_I32_TYPE_IDENTITY,
    MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY,
    MACOS_X86_64_ADDRESS_TYPE_IDENTITY, MACOS_X86_64_I32_TYPE_IDENTITY,
    ProgramEntryPhysicalContractPackageSourceDigest, ProgramEntryPhysicalContractPlan,
    WINDOWS_X86_64_U32_TYPE_IDENTITY, exact_hosted_physical_boundary_entry_plan,
    exact_physical_contract_package_source_digest,
};
pub use selected_entry::{ProgramStorageEntryRootRole, SelectedProgramStorageEntryPlan};

const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;

pub const fn uefi_os_handoff_exhaustion_requires_error(exhaustion_status: u64) -> bool {
    exhaustion_status & EFI_STATUS_ERROR_BIT != 0
}
