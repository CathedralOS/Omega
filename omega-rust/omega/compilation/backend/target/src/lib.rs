//! Target descriptions: deployment profiles, semantics, x86 feature sets,
//! ELF loading, UEFI tables.
//!
//! Start at `target_profile.rs`: the deployment-profile catalog (`TargetProfile`,
//! `NativeTarget`, and the program-entry slot contract) that every other area
//! names. `target_semantics` names each supported native target's observation
//! promises; `x86_features` records feature requirements; `elf_loader` and the
//! `uefi_*` folders describe the loader and firmware structures a program may
//! be handed, each with its occurrence carrier beside it; `foreign_locator`
//! identifies foreign providers. Emission reads these; none of them emits
//! bytes.
//! `application` owns presentation intent and deployment identity shared by Build
//! extraction, signed images, and application-package publication.

mod application;
pub mod compatibility_fingerprint;
mod elf_loader;
mod explicit_target_set;
mod foreign_locator;
mod target_profile;
mod target_semantics;
mod uefi_boot_services;
mod uefi_loaded_image;
mod uefi_system_table;
mod x86_features;

pub use application::{ApplicationIdentifier,HostedApplicationIntent,NativeApplicationMetadata};
pub use elf_loader::{ELF_INTERPRETER_TARGET_PROFILES,NormalizedElfInterpreterPlan,declared_elf_interpreter_default,normalize_elf_interpreter_plan};
pub use explicit_target_set::ExplicitTargetSet;
pub use foreign_locator::{ForeignLocatorCandidate,ForeignLocatorIdentityDigest,ForeignLocatorValidationError,NormalizedForeignLocator,evaluated_syscall_identity_digest,normalize_foreign_locator};
pub use target_profile::{Architecture,HostedIntrinsicBundle,NativeTarget,ObjectFormat,ProgramEntryCallingConvention,ProgramEntryPhysicalContractPackage,ProgramEntryReceiverProvisioning,ProgramEntrySchema,ProgramEntrySlotDeclaration,ProgramEntryVisibleParameters,TargetProfile};
pub use target_semantics::{SymbolicTargetObservationApplication,TargetEntryStackGuarantee,TargetSemantics,UefiX86_64};
pub use uefi_boot_services::{UEFI_LOADED_IMAGE_PROTOCOL_GUID};
pub(crate) use uefi_boot_services::{UefiBootServicesNativeField,UefiBootServicesNativeFieldKind,UefiBootServicesNativeFieldLayout,ValidatedUefiBootServicesNativeLayout};
pub(crate) use uefi_loaded_image::{UefiLoadedImageNativeField,UefiLoadedImageNativeFieldKind,UefiLoadedImageNativeFieldLayout,ValidatedUefiLoadedImageNativeLayout,exact_uefi_x64_loaded_image_native_layout};
pub(crate) use uefi_system_table::{UefiSystemTableNativeField,UefiSystemTableNativeFieldKind,UefiSystemTableNativeFieldLayout,ValidatedUefiSystemTableNativeLayout};
pub use x86_features::{AdmittedX86ScalarFmaProvider,X86_SCALAR_FMA_REQUIRED_FEATURES,X86DeploymentFeatures,X86FeatureRequirement,X86ScalarFmaSlot};

pub(crate) fn require(
    condition: bool,
    message: &'static str,
) -> Result<(), diagnostics::Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| diagnostics::Diagnostic::error(message))
}

pub(crate) fn validate_uefi_x64_target_owner(
    profile: TargetProfile,
    entry_slot: crate::ProgramEntrySlotDeclaration,
    layout_name: &'static str,
    entry_drift: &'static str,
) -> Result<(), diagnostics::Diagnostic> {
    let target = profile.native_target();
    require(
        profile == TargetProfile::UefiX64
            && target.architecture == crate::Architecture::X86_64
            && target.object_format == crate::ObjectFormat::Coff
            && target.pointer_size == 8
            && target.pointer_alignment == 8,
        match layout_name {
            "EFI_BOOT_SERVICES" => {
                "EFI_BOOT_SERVICES native layout is owned only by the UEFI x86-64 target"
            }
            "EFI_LOADED_IMAGE_PROTOCOL" => {
                "EFI_LOADED_IMAGE_PROTOCOL native layout is owned only by the UEFI x86-64 target"
            }
            _ => "EFI_SYSTEM_TABLE native layout is owned only by the UEFI x86-64 target",
        },
    )?;
    require(
        entry_slot == TargetProfile::UefiX64.program_entry_slot()
            && entry_slot.physical_arrival_requirement == Some("UefiPhysicalEntry::enter")
            && entry_slot.physical_contract_package
                == Some(crate::ProgramEntryPhysicalContractPackage::UefiX64)
            && entry_slot.physical_calling_convention
                == Some(crate::ProgramEntryCallingConvention::MicrosoftX64),
        entry_drift,
    )
}
