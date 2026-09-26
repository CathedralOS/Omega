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

mod elf_loader;
mod foreign_locator;
mod target_profile;
mod target_semantics;
mod uefi_boot_services;
mod uefi_loaded_image;
mod uefi_system_table;
mod x86_features;

pub use elf_loader::{
    ElfInterpreterPlanValidationError, NormalizedElfInterpreterPlan, normalize_elf_interpreter_plan,
};
pub use foreign_locator::{
    ForeignLocatorCandidate, ForeignLocatorIdentityDigest, ForeignLocatorValidationError,
    NormalizedForeignLocator, evaluated_syscall_identity_digest, normalize_foreign_locator,
};
pub use target_profile::{
    Architecture, HostedIntrinsicBundle, NativeTarget, ObjectFormat, ProgramEntryCallingConvention,
    ProgramEntryPhysicalContractPackage, ProgramEntryReceiverProvisioning, ProgramEntrySchema,
    ProgramEntrySlotDeclaration, ProgramEntryVisibleParameters, TargetProfile,
    TargetProfileIdentity, TargetRequiredRootSlotDeclaration,
};
pub use target_semantics::{
    SymbolicTargetObservationApplication, TargetEntryStackGuarantee, TargetEntryStackSubject,
    TargetSemanticObservationError, TargetSemantics, UefiX86_64,
};
pub use uefi_boot_services::occurrence::{
    UEFI_BOOT_SERVICES_SIGNATURE, UefiBootServicesOccurrenceValidationError,
    ValidatedUefiBootServicesHeaderIntegrity, validate_uefi_boot_services_occurrence,
};
pub use uefi_boot_services::{
    UEFI_LOADED_IMAGE_PROTOCOL_GUID, UEFI_X64_BOOT_SERVICES_LAYOUT_PLAN_COMMITMENT,
    UEFI_X64_BOOT_SERVICES_NATIVE_LAYOUT_COMMITMENT,
    UEFI_X64_BOOT_SERVICES_SCHEMA_REPORT_FINGERPRINT, UefiBootServicesNativeField,
    UefiBootServicesNativeFieldKind, UefiBootServicesNativeFieldLayout,
    UefiBootServicesNativeLayoutError, UefiProtocolGuid, ValidatedUefiBootServicesNativeLayout,
    exact_uefi_x64_boot_services_layout_plan_report, exact_uefi_x64_boot_services_native_layout,
    plan_uefi_boot_services_native_layout, replayed_uefi_x64_boot_services_native_layout,
};
pub use uefi_loaded_image::occurrence::{
    UEFI_LOADED_IMAGE_PROTOCOL_REVISION, UefiLoadedImageOccurrenceValidationError,
    ValidatedUefiLoadedImageGeometry, validate_uefi_loaded_image_occurrence,
};
pub use uefi_loaded_image::{
    UEFI_X64_LOADED_IMAGE_LAYOUT_PLAN_COMMITMENT, UEFI_X64_LOADED_IMAGE_NATIVE_LAYOUT_COMMITMENT,
    UEFI_X64_LOADED_IMAGE_SCHEMA_REPORT_FINGERPRINT, UefiLoadedImageNativeField,
    UefiLoadedImageNativeFieldKind, UefiLoadedImageNativeFieldLayout,
    ValidatedUefiLoadedImageNativeLayout, exact_uefi_x64_loaded_image_layout_plan_report,
    exact_uefi_x64_loaded_image_native_layout, replayed_uefi_x64_loaded_image_native_layout,
};
pub use uefi_system_table::occurrence::{
    UEFI_SYSTEM_TABLE_SIGNATURE, UefiSystemTableOccurrenceValidationError,
    ValidatedUefiSystemTableHeaderIntegrity, validate_uefi_system_table_occurrence,
};
pub use uefi_system_table::{
    UEFI_X64_SYSTEM_TABLE_LAYOUT_PLAN_COMMITMENT, UEFI_X64_SYSTEM_TABLE_NATIVE_LAYOUT_COMMITMENT,
    UEFI_X64_SYSTEM_TABLE_SCHEMA_REPORT_FINGERPRINT, UefiSystemTableNativeField,
    UefiSystemTableNativeFieldKind, UefiSystemTableNativeFieldLayout,
    UefiSystemTableNativeLayoutError, ValidatedUefiSystemTableNativeLayout,
    exact_uefi_x64_system_table_layout_plan_report, exact_uefi_x64_system_table_native_layout,
    plan_uefi_system_table_native_layout, replayed_uefi_x64_system_table_native_layout,
};
pub use x86_features::{
    AdmittedX86ScalarFmaProvider, X86_SCALAR_FMA_REQUIRED_FEATURES, X86DeploymentFeatures,
    X86FeatureRequirement, X86ScalarFmaAdmissionError, X86ScalarFmaDifferentialReceipt,
    X86ScalarFmaSlot, X86TargetFeature,
};
