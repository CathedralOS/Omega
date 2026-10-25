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

pub(crate) use optimization_core::codec_bytes;

mod application;
pub mod compatibility_fingerprint;
mod component_locator;
pub(crate) use optimization_core::digest_encoder;
mod elf_loader;
mod explicit_target_set;
mod foreign_locator;
mod target_profile;
mod target_semantics;
mod x86_features;

pub use application::{ApplicationIdentifier, HostedApplicationIntent, NativeApplicationMetadata};
pub use component_locator::{
    ImportLocatorIdentityDigest, NormalizedComponentLocator, NormalizedImportLocator,
    NormalizedImportLocatorRef, component_export_slot,
};
pub use elf_loader::{
    ELF_INTERPRETER_TARGET_PROFILES, NormalizedElfInterpreterPlan,
    declared_elf_interpreter_default, normalize_elf_interpreter_plan,
};
pub use explicit_target_set::ExplicitTargetSet;
pub use foreign_locator::{
    ForeignLocatorCandidate, ForeignLocatorIdentityDigest, ForeignLocatorValidationError,
    NormalizedForeignLocator, evaluated_syscall_identity_digest, normalize_foreign_locator,
};
pub use target_profile::{
    Architecture, HostedIntrinsicBundle, NativeTarget, ObjectFormat, PhysicalLayoutContext,
    ProgramEntryCallingConvention, ProgramEntryPhysicalContractPackage, ProgramEntrySchema,
    ProgramEntrySlotDeclaration, ProgramEntryVisibleParameters, TargetProfile,
    TargetWireIdentityError,
};
pub use target_semantics::{
    SymbolicTargetObservationApplication, TargetEntryStackGuarantee, TargetSemantics, UefiX86_64,
};
pub use x86_features::{
    AdmittedX86ScalarFmaProvider, X86_SCALAR_FMA_REQUIRED_FEATURES, X86DeploymentFeatures,
    X86FeatureRequirement, X86ScalarFmaSlot,
};
