/*
We define the target-description vocabulary for native compilation: deployment
profiles (`TargetProfile`), realized architecture and object-format shapes
(`NativeTarget`), target-owned program-entry slot contracts, symbolic target
observation promises (`target_semantics`), x86 feature requirements
(`x86_features`), ELF interpreter plans (`elf_loader`), normalized foreign and
component import locators (`foreign_locator`, `component_locator`), and
authored application presentation metadata (`HostedApplicationIntent`,
`NativeApplicationMetadata`, `ApplicationIdentifier`).
*/

/*
We keep target descriptions separate from byte emission, source evaluation, and
host inspection so every downstream stage consumes one closed catalog of
target identities and physical boundary requirements without inferring target
policy from the executing host or re-parsing raw locator strings.
*/

/*
Start at `target_profile.rs` for the canonical deployment-profile catalog and
program-entry slot declarations. `target_semantics.rs` closes symbolic target
observations such as the UEFI entry-stack guarantee, `x86_features.rs` binds
feature requirements and differential receipts to exact profiles, and
`elf_loader.rs`, `foreign_locator.rs`, and `component_locator.rs` normalize
loader and import coordinates with length-framed compatibility fingerprints
from `compatibility_fingerprint.rs`.
*/

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;

pub mod compatibility_fingerprint;
mod component_locator;
mod elf_loader;
mod explicit_target_set;
mod float_realization;
mod foreign_locator;
mod target_profile;
mod target_semantics;
mod x86_features;

pub use component_locator::{
    ImportLocatorIdentityDigest, NormalizedComponentLocator, NormalizedImportLocator,
    NormalizedImportLocatorRef, component_export_slot,
};
pub use elf_loader::{
    ELF_INTERPRETER_TARGET_PROFILES, NormalizedElfInterpreterPlan,
    declared_elf_interpreter_default, normalize_elf_interpreter_plan,
};
pub use explicit_target_set::ExplicitTargetSet;
pub use float_realization::{
    CANONICAL_FLOAT_OPERATIONS, CanonicalFloatRealization, FloatControlObligation,
    FloatIsaAdmission, canonical_float_operation_name,
};
pub use foreign_locator::{
    ForeignLocatorCandidate, ForeignLocatorIdentityDigest, ForeignLocatorValidationError,
    NormalizedForeignLocator, evaluated_syscall_identity_digest, normalize_foreign_locator,
};
pub use target_profile::{
    Architecture, CASE_TAG_BYTE_ALIGNMENT, CASE_TAG_BYTE_SIZE, HostedIntrinsicBundle, NativeTarget,
    ObjectFormat, ProgramEntryCallingConvention, ProgramEntryPhysicalContractPackage,
    ProgramEntrySchema, ProgramEntrySlotDeclaration, ProgramEntrySlots,
    ProgramEntryVisibleParameters, TargetProfile, TargetWireIdentityError,
};
pub use target_semantics::{
    SymbolicTargetObservationApplication, TargetEntryStackGuarantee, TargetSemantics, UefiX86_64,
};
pub use x86_features::{
    AdmittedX86ScalarFmaProvider, X86_SCALAR_FMA_REQUIRED_FEATURES, X86DeploymentFeatures,
    X86FeatureRequirement, X86ScalarFmaDifferentialReceipt, X86ScalarFmaSlot, X86TargetFeature,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedApplicationIntent {
    Console,
    Gui,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeApplicationMetadata {
    pub name: Option<String>, // Validated authored application name; absent for other project roles.
    pub intent: Option<HostedApplicationIntent>,
    pub identifier: Option<ApplicationIdentifier>, // Identity bound into signing and application-package publication.
    pub subsystem: u16,
    pub elf_interpreter: Option<crate::NormalizedElfInterpreterPlan>, // Effective ELF loader of the realized target, or `None` for targets without `PT_INTERP`.
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApplicationIdentifier(String);

impl ApplicationIdentifier {
    pub fn new(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.is_empty() {
            return Err("identifier is empty");
        }
        if !bytes.is_ascii() {
            return Err("identifier is not ASCII");
        }
        if bytes.iter().any(|byte| {
            !matches!(
                byte,
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'
            )
        }) {
            return Err("identifier contains a byte outside `A-Z a-z 0-9 . -`");
        }
        if bytes
            .split(|byte| *byte == b'.')
            .any(|segment| segment.is_empty())
        {
            return Err("identifier has an empty `.`-separated segment");
        }
        Ok(Self(
            String::from_utf8(bytes.to_vec()).expect("ASCII identifier bytes are UTF-8"),
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
