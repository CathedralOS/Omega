//! Target descriptions: semantics, x86 feature sets, ELF loading, UEFI tables.
//!
//! Start at `target_semantics.rs`: it names each supported native target and
//! what it promises. `x86_features` records feature requirements; `elf_loader`
//! and the `uefi_*` folders describe the loader and firmware structures a
//! program may be handed, each with its occurrence carrier beside it;
//! `foreign_locator` identifies foreign providers. Emission reads these; none
//! of them emits bytes.

use diagnostics::Diagnostic;

mod elf_loader;
mod foreign_locator;
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
pub use target_semantics::{
    SymbolicTargetObservationApplication, TargetEntryStackGuarantee, TargetEntryStackSubject,
    TargetSemanticObservationError, TargetSemantics, UefiX86_64,
};
pub use uefi_boot_services::occurrence::{
    UEFI_BOOT_SERVICES_SIGNATURE, UefiBootServicesOccurrenceValidationError,
    ValidatedUefiBootServicesHeaderIntegrity, validate_uefi_boot_services_occurrence,
};
pub use uefi_boot_services::{
    UEFI_LOADED_IMAGE_PROTOCOL_GUID, UefiBootServicesNativeField, UefiBootServicesNativeFieldKind,
    UefiBootServicesNativeFieldLayout, UefiBootServicesNativeLayoutError, UefiProtocolGuid,
    ValidatedUefiBootServicesNativeLayout, plan_uefi_boot_services_native_layout,
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
    UefiSystemTableNativeField, UefiSystemTableNativeFieldKind, UefiSystemTableNativeFieldLayout,
    UefiSystemTableNativeLayoutError, ValidatedUefiSystemTableNativeLayout,
    plan_uefi_system_table_native_layout,
};
pub use x86_features::{
    AdmittedX86ScalarFmaProvider, X86_SCALAR_FMA_REQUIRED_FEATURES, X86DeploymentFeatures,
    X86FeatureRequirement, X86ScalarFmaAdmissionError, X86ScalarFmaDifferentialReceipt,
    X86ScalarFmaSlot, X86TargetFeature,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    Aarch64,
    X86_64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    Elf,
    MachO,
    Coff,
}

/// The selected deployment profile. Unlike [`NativeTarget`], this identity
/// retains policy distinctions that share one architecture/object format
/// (notably Windows and UEFI x86-64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetProfile {
    LinuxArm64,
    LinuxX64,
    MacosArm64,
    WindowsX64,
    UefiX64,
    CrossPlatformCli,
    LocalUnchecked,
    /// The bootstrap Alpha target. Recognition makes its profile and slot
    /// identities ordinary target-catalog facts while realization stays
    /// with the bootstrap chain's own compilers: an inactive
    /// `alpha_bootstrap` root binding must not demand a backend this
    /// compiler does not have, and selecting the profile reports not
    /// implemented rather than a checked result or a fallback artifact.
    AlphaBootstrap,
}

/// Stable, domain-separated identity of one deployment profile.
///
/// Source spelling and enum order are presentation details. Retained target
/// evidence uses this identity so those details cannot silently rename a
/// profile already present in locks or review material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetProfileIdentity(&'static str);

impl TargetProfileIdentity {
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntrySchema {
    /// Hosted launch hides physical image/storage arrival from source.
    HostedApplication,
    /// Freestanding launch exposes the image and initial-storage roots.
    ProgramStorageApplication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntryVisibleParameters {
    None,
    ImageAndInitialStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntryReceiverProvisioning {
    /// The selected source machine may be free or request one ZII-valid
    /// exclusive receiver occurrence from the generated bridge.
    NoneOrProvisionedZii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntryCallingConvention {
    MicrosoftX64,
    Aapcs64,
    SystemVAMD64,
}

/// Closed identity of a toolchain-owned physical-entry contract package.
/// Source paths prove membership in this identity; they never define it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntryPhysicalContractPackage {
    UefiX64,
    MacosArm64,
    LinuxX86_64,
    LinuxArm64,
    WindowsX64,
}

impl ProgramEntryPhysicalContractPackage {
    pub const fn manifest_identity(self) -> &'static str {
        match self {
            Self::UefiX64 => "omega::language::std::targets::uefi_x86_64::entry",
            Self::MacosArm64 => "omega::language::std::targets::macos_arm64::entry",
            Self::LinuxX86_64 => "omega::language::std::targets::linux_x86_64::entry",
            Self::LinuxArm64 => "omega::language::std::targets::linux_arm64::entry",
            Self::WindowsX64 => "omega::language::std::targets::windows_x86_64::entry",
        }
    }

    pub const fn package_relative_source(self) -> &'static str {
        match self {
            Self::UefiX64 => "targets/uefi_x86_64/entry.omg",
            Self::MacosArm64 => "targets/macos_arm64/entry.omg",
            Self::LinuxX86_64 => "targets/linux_x86_64/entry.omg",
            Self::LinuxArm64 => "targets/linux_arm64/entry.omg",
            Self::WindowsX64 => "targets/windows_x86_64/entry.omg",
        }
    }

    /// Diagnostic spelling of the contract this package owns. Identity still
    /// comes from the closed variant and manifest identity, never this name.
    pub const fn contract_name(self) -> &'static str {
        match self {
            Self::UefiX64 => "UEFI",
            Self::MacosArm64 => "macOS ARM64",
            Self::LinuxX86_64 => "Linux x86-64",
            Self::LinuxArm64 => "Linux ARM64",
            Self::WindowsX64 => "Windows x86-64",
        }
    }
}

/// Target-owned declaration of the first environment-to-program root slot.
/// The source binding supplies only `machine`; every other field belongs to
/// the selected target profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntrySlotDeclaration {
    pub owner: TargetProfile,
    pub slot_name: &'static str,
    pub schema: ProgramEntrySchema,
    /// Stable semantic installation edge. This is never the platform ABI.
    pub semantic_arrival_requirement: &'static str,
    /// Target-fixed physical environment arrival. Hosted compatibility
    /// profiles retain `None` until their two-surface entry bridge lands.
    pub physical_arrival_requirement: Option<&'static str>,
    /// Exact closed toolchain package that owns the physical requirement.
    pub physical_contract_package: Option<ProgramEntryPhysicalContractPackage>,
    /// Source boundary schema whose evaluated `Calling<C>` plan owns the
    /// physical entry contract. `None` marks a profile not yet migrated from
    /// its hosted compatibility bridge.
    pub boundary_schema: Option<&'static str>,
    /// Calling convention for the target-fixed physical arrival.
    pub physical_calling_convention: Option<ProgramEntryCallingConvention>,
    /// Private ABI used by the generated bridge to call the selected semantic
    /// continuation. It is deliberately distinct from physical arrival even
    /// when both currently select Microsoft x64.
    pub semantic_calling_convention: Option<ProgramEntryCallingConvention>,
    pub visible_parameters: ProgramEntryVisibleParameters,
    pub receiver: ProgramEntryReceiverProvisioning,
}

/// One build-bound environment-to-program root required by a target profile.
///
/// The catalog is intentionally an enum rather than a flattened bag of entry
/// fields: each slot schema owns its own physical/semantic contract shape.
/// Consumers that only implement one schema must inspect the variant and fail
/// closed for every other member instead of accepting and ignoring it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRequiredRootSlotDeclaration {
    ProgramEntry(ProgramEntrySlotDeclaration),
}

impl TargetRequiredRootSlotDeclaration {
    pub const fn owner(self) -> TargetProfile {
        match self {
            Self::ProgramEntry(slot) => slot.owner,
        }
    }

    pub const fn slot_name(self) -> &'static str {
        match self {
            Self::ProgramEntry(slot) => slot.slot_name,
        }
    }

    pub const fn program_entry(self) -> Option<ProgramEntrySlotDeclaration> {
        match self {
            Self::ProgramEntry(slot) => Some(slot),
        }
    }
}

impl From<ProgramEntrySlotDeclaration> for TargetRequiredRootSlotDeclaration {
    fn from(slot: ProgramEntrySlotDeclaration) -> Self {
        Self::ProgramEntry(slot)
    }
}

impl TargetProfile {
    /// Complete trusted deployment-profile catalog in canonical identity order.
    ///
    /// Consumers that retain profile-indexed evidence must use this catalog
    /// rather than maintaining a parallel list that can drift from the
    /// compiler's accepted source-visible cases.
    pub const ALL: [Self; 8] = [
        Self::LinuxArm64,
        Self::LinuxX64,
        Self::MacosArm64,
        Self::WindowsX64,
        Self::UefiX64,
        Self::CrossPlatformCli,
        Self::LocalUnchecked,
        Self::AlphaBootstrap,
    ];

    pub const fn identity(self) -> TargetProfileIdentity {
        TargetProfileIdentity(match self {
            Self::LinuxArm64 => "omega.target-profile.v1:linux_arm64",
            Self::LinuxX64 => "omega.target-profile.v1:linux_x86_64",
            Self::MacosArm64 => "omega.target-profile.v1:macos_arm64",
            Self::WindowsX64 => "omega.target-profile.v1:windows_x86_64",
            Self::UefiX64 => "omega.target-profile.v1:uefi_x86_64",
            Self::CrossPlatformCli => "omega.target-profile.v1:cross_platform_cli",
            Self::LocalUnchecked => "omega.target-profile.v1:local_unchecked",
            Self::AlphaBootstrap => "omega.target-profile.v1:alpha_bootstrap",
        })
    }

    pub fn host() -> Self {
        Self::host_if_supported().expect("unsupported host profile for Omega native planning")
    }

    /// The catalogued deployment profile this host admits, when one exists.
    ///
    /// A host with a usable `NativeTarget` triple but no catalogued profile
    /// (macOS x86-64, for example) returns `None`. Callers that admit the
    /// compiler host for interpreted work — build-scope source selection and
    /// build machine execution — must carry that absence rather than panic or
    /// name a foreign profile.
    pub fn host_if_supported() -> Option<Self> {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Self::MacosArm64)
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Some(Self::LinuxArm64)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Self::LinuxX64)
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(Self::WindowsX64)
        } else {
            None
        }
    }

    pub fn from_omega_target_name(target_name: Option<&str>) -> Result<Self, Diagnostic> {
        let Some(target_name) = target_name else {
            return Ok(Self::host());
        };
        Self::ALL
            .into_iter()
            .find(|profile| {
                profile.target_name() == target_name
                    || profile.legacy_cli_alias() == Some(target_name)
            })
            .ok_or_else(|| Diagnostic::error(format!(
                "unknown target profile `{target_name}`; expected linux_arm64, linux_x86_64, macos_arm64, windows_x86_64, uefi_x86_64, cross_platform_cli, local_unchecked, or alpha_bootstrap"
            )))
    }

    /// Parse a canonical target identity outside the invocation boundary.
    /// Transitional CLI aliases must normalize before entering source selection,
    /// locks, review evidence, or semantic identity.
    pub fn from_canonical_target_name(target_name: &str) -> Result<Self, Diagnostic> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.target_name() == target_name)
            .ok_or_else(|| {
                Diagnostic::error(format!("unknown canonical target profile `{target_name}`"))
            })
    }

    /// Parse the canonical source-level owner of a target-declared slot. This
    /// namespace is intentionally distinct from deployment CLI target names.
    pub fn from_root_slot_owner(owner: &str) -> Result<Self, Diagnostic> {
        Self::from_canonical_target_name(owner)
            .map_err(|_| Diagnostic::error(format!("unknown target root-slot owner `{owner}`")))
    }

    pub const fn target_name(self) -> &'static str {
        match self {
            Self::LinuxArm64 => "linux_arm64",
            Self::LinuxX64 => "linux_x86_64",
            Self::MacosArm64 => "macos_arm64",
            Self::WindowsX64 => "windows_x86_64",
            Self::UefiX64 => "uefi_x86_64",
            Self::CrossPlatformCli => "cross_platform_cli",
            Self::LocalUnchecked => "local_unchecked",
            Self::AlphaBootstrap => "alpha_bootstrap",
        }
    }

    const fn legacy_cli_alias(self) -> Option<&'static str> {
        match self {
            Self::LinuxX64 => Some("linux_x64"),
            Self::WindowsX64 => Some("windows_x64"),
            Self::UefiX64 => Some("uefi_x64"),
            Self::LinuxArm64
            | Self::MacosArm64
            | Self::CrossPlatformCli
            | Self::LocalUnchecked
            | Self::AlphaBootstrap => None,
        }
    }

    /// Canonical source-visible case supplied through the compiler-owned
    /// `Build.target` field. This identity is distinct from transitional CLI
    /// spellings and target root-slot owner namespaces.
    pub const fn build_case_name(self) -> &'static str {
        match self {
            Self::LinuxArm64 => "LinuxArm64",
            Self::LinuxX64 => "LinuxX86_64",
            Self::MacosArm64 => "MacosArm64",
            Self::WindowsX64 => "WindowsX86_64",
            Self::UefiX64 => "UefiX86_64",
            Self::CrossPlatformCli => "CrossPlatformCli",
            Self::LocalUnchecked => "LocalUnchecked",
            Self::AlphaBootstrap => "AlphaBootstrap",
        }
    }

    pub fn from_build_case_name(case: &str) -> Option<Self> {
        match case {
            "LinuxArm64" => Some(Self::LinuxArm64),
            "LinuxX86_64" => Some(Self::LinuxX64),
            "MacosArm64" => Some(Self::MacosArm64),
            "WindowsX86_64" => Some(Self::WindowsX64),
            "UefiX86_64" => Some(Self::UefiX64),
            "CrossPlatformCli" => Some(Self::CrossPlatformCli),
            "LocalUnchecked" => Some(Self::LocalUnchecked),
            "AlphaBootstrap" => Some(Self::AlphaBootstrap),
            _ => None,
        }
    }

    pub const fn root_slot_owner_name(self) -> &'static str {
        match self {
            Self::LinuxArm64 => "linux_arm64",
            Self::LinuxX64 => "linux_x86_64",
            Self::MacosArm64 => "macos_arm64",
            Self::WindowsX64 => "windows_x86_64",
            Self::UefiX64 => "uefi_x86_64",
            Self::CrossPlatformCli => "cross_platform_cli",
            Self::LocalUnchecked => "local_unchecked",
            Self::AlphaBootstrap => "alpha_bootstrap",
        }
    }

    /// The Rust compiler's native realization for this profile when it has
    /// one. Recognition is not implementation: profiles realized outside
    /// this compiler, like `alpha_bootstrap`, return `None`, while
    /// `native_target()` stays total over natively realized profiles.
    pub fn native_realization(self) -> Option<NativeTarget> {
        match self {
            Self::AlphaBootstrap => None,
            profile => Some(profile.native_target()),
        }
    }

    pub fn native_target(self) -> NativeTarget {
        match self {
            Self::LinuxArm64 => NativeTarget::linux_arm64(),
            Self::LinuxX64 => NativeTarget::linux_x64(),
            Self::MacosArm64 => NativeTarget::macos_arm64(),
            Self::WindowsX64 => NativeTarget::windows_x64(),
            Self::UefiX64 => NativeTarget::uefi_x64(),
            Self::CrossPlatformCli | Self::LocalUnchecked => NativeTarget::host(),
            // Alpha has no `NativeTarget`: no architecture/object-format
            // pair describes the bootstrap target, and selection rejects it
            // through `NativeTarget::from_omega_target_name` before any
            // caller reaches native planning.
            Self::AlphaBootstrap => {
                unreachable!("alpha_bootstrap has no Rust native realization")
            }
        }
    }

    pub const fn program_entry_slot(self) -> ProgramEntrySlotDeclaration {
        let (
            schema,
            visible_parameters,
            boundary_schema,
            physical_arrival_requirement,
            physical_contract_package,
            physical_calling_convention,
            semantic_calling_convention,
        ) = match self {
            Self::UefiX64 => (
                ProgramEntrySchema::ProgramStorageApplication,
                ProgramEntryVisibleParameters::ImageAndInitialStorage,
                Some("UefiApplication"),
                Some("UefiPhysicalEntry::enter"),
                Some(ProgramEntryPhysicalContractPackage::UefiX64),
                Some(ProgramEntryCallingConvention::MicrosoftX64),
                Some(ProgramEntryCallingConvention::MicrosoftX64),
            ),
            // The macOS ARM64 hosted bridge retains two authored entry
            // surfaces in toolchain custody: `MacosPhysicalEntry::enter` is
            // the physical process arrival and `ProgramStorageEntry::enter`
            // is the semantic continuation it must adapter-map into. The
            // source-visible application stays `HostedApplication` with no
            // authored storage parameters; the two internal roots are
            // provisioned by the bridge, never hosted arguments.
            Self::MacosArm64 => (
                ProgramEntrySchema::HostedApplication,
                ProgramEntryVisibleParameters::None,
                Some("MacosApplication"),
                Some("MacosPhysicalEntry::enter"),
                Some(ProgramEntryPhysicalContractPackage::MacosArm64),
                Some(ProgramEntryCallingConvention::Aapcs64),
                Some(ProgramEntryCallingConvention::Aapcs64),
            ),
            // The Linux x86-64 hosted bridge retains the same two authored
            // surfaces: `LinuxPhysicalEntry::enter` is the kernel process
            // arrival (initial stack image in rsp, exit_group status in edi)
            // and `ProgramStorageEntry::enter` is the semantic continuation it
            // adapter-maps into. The source-visible application stays
            // `HostedApplication` with no authored storage parameters; the two
            // internal roots are provisioned by the bridge, never hosted
            // arguments.
            Self::LinuxX64 => (
                ProgramEntrySchema::HostedApplication,
                ProgramEntryVisibleParameters::None,
                Some("LinuxX86_64Application"),
                Some("LinuxPhysicalEntry::enter"),
                Some(ProgramEntryPhysicalContractPackage::LinuxX86_64),
                Some(ProgramEntryCallingConvention::SystemVAMD64),
                Some(ProgramEntryCallingConvention::SystemVAMD64),
            ),
            // The Linux ARM64 hosted bridge retains the same two authored
            // surfaces: `LinuxPhysicalEntry::enter` is the kernel process
            // arrival (the initial process-stack image's argument-count head
            // word is delivered at the incoming stack base; completion leaves
            // through the exit_group supervisor call with status in w0) and
            // `ProgramStorageEntry::enter` is the semantic continuation it
            // adapter-maps into. The source-visible application stays
            // `HostedApplication` with no authored storage parameters; the two
            // internal roots are provisioned by the bridge, never hosted
            // arguments.
            Self::LinuxArm64 => (
                ProgramEntrySchema::HostedApplication,
                ProgramEntryVisibleParameters::None,
                Some("LinuxArm64Application"),
                Some("LinuxPhysicalEntry::enter"),
                Some(ProgramEntryPhysicalContractPackage::LinuxArm64),
                Some(ProgramEntryCallingConvention::Aapcs64),
                Some(ProgramEntryCallingConvention::Aapcs64),
            ),
            // The Windows x86-64 hosted bridge retains the same two authored
            // surfaces: `WindowsProcessEntry::enter` is the loader process
            // arrival (no contractual register inputs, 32-byte shadow space,
            // completion status returned in eax as the process exit code) and
            // `ProgramStorageEntry::enter` is the semantic continuation it
            // adapter-maps into. The source-visible application stays
            // `HostedApplication` with no authored storage parameters; the two
            // internal roots are provisioned by the bridge, never hosted
            // arguments.
            Self::WindowsX64 => (
                ProgramEntrySchema::HostedApplication,
                ProgramEntryVisibleParameters::None,
                Some("WindowsX86_64Application"),
                Some("WindowsProcessEntry::enter"),
                Some(ProgramEntryPhysicalContractPackage::WindowsX64),
                Some(ProgramEntryCallingConvention::MicrosoftX64),
                Some(ProgramEntryCallingConvention::MicrosoftX64),
            ),
            _ => (
                ProgramEntrySchema::HostedApplication,
                ProgramEntryVisibleParameters::None,
                None,
                None,
                None,
                None,
                None,
            ),
        };
        ProgramEntrySlotDeclaration {
            owner: self,
            slot_name: "ProgramEntry",
            schema,
            semantic_arrival_requirement: "ProgramStorageEntry::enter",
            physical_arrival_requirement,
            physical_contract_package,
            boundary_schema,
            physical_calling_convention,
            semantic_calling_convention,
            visible_parameters,
            receiver: ProgramEntryReceiverProvisioning::NoneOrProvisionedZii,
        }
    }

    /// Complete ordered catalog of build-bound external roots required by
    /// this target profile. Runtime-open roots (for example dynamically
    /// installed callbacks or interrupt vectors) do not belong here.
    ///
    /// `ProgramEntry` is the sole current member. Returning a catalog now,
    /// rather than teaching closure verification that singleton fact, makes a
    /// later real target-owned member visible to every completeness consumer.
    pub fn required_root_slots(
        self,
    ) -> impl ExactSizeIterator<Item = TargetRequiredRootSlotDeclaration> {
        [TargetRequiredRootSlotDeclaration::ProgramEntry(
            self.program_entry_slot(),
        )]
        .into_iter()
    }

    pub fn required_root_slot(self, slot_name: &str) -> Option<TargetRequiredRootSlotDeclaration> {
        self.required_root_slots()
            .find(|slot| slot.slot_name() == slot_name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeTarget {
    pub architecture: Architecture,
    pub object_format: ObjectFormat,
    pub pointer_size: usize,
    pub pointer_alignment: usize,
}

impl NativeTarget {
    pub fn host() -> Self {
        Self {
            architecture: host_architecture(),
            object_format: host_object_format(),
            pointer_size: std::mem::size_of::<usize>(),
            pointer_alignment: std::mem::align_of::<usize>(),
        }
    }

    pub fn from_omega_target_name(target_name: Option<&str>) -> Result<Self, Diagnostic> {
        match target_name {
            None => Ok(Self::host()),
            Some(target_name) => match TargetProfile::from_omega_target_name(Some(target_name))? {
                // A recognized profile is not an implemented one: profile
                // identity is a target-catalog fact, while this function is
                // where a demanded operation acquires a native realization.
                // `alpha_bootstrap` has none, so selecting it reports not
                // implemented instead of an unknown-name or a host fallback.
                TargetProfile::AlphaBootstrap => Err(Diagnostic::error(format!(
                    "native realization for target profile `{target_name}` is not implemented"
                ))),
                profile => Ok(profile.native_target()),
            },
        }
    }

    pub fn linux_arm64() -> Self {
        Self {
            architecture: Architecture::Aarch64,
            object_format: ObjectFormat::Elf,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    }

    pub fn linux_x64() -> Self {
        Self {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Elf,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    }

    pub fn macos_arm64() -> Self {
        Self {
            architecture: Architecture::Aarch64,
            object_format: ObjectFormat::MachO,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    }

    pub fn windows_x64() -> Self {
        Self {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Coff,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    }

    /// The UEFI application target: x86_64 PE32+ (the Coff/PE emitter),
    /// matching the boot-verified milestone-1 image shape. The
    /// subsystem-10/freestanding facts come from build.omg
    /// (`efi_application`), exactly as they did on a Windows host; this
    /// entry only pins architecture + format so the efi family
    /// cross-compiles from ANY host. The name was already load-bearing in
    /// source (`uefi_x64` external leaves, D15).
    pub fn uefi_x64() -> Self {
        Self {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Coff,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    }
}

fn host_architecture() -> Architecture {
    if cfg!(target_arch = "aarch64") {
        Architecture::Aarch64
    } else if cfg!(target_arch = "x86_64") {
        Architecture::X86_64
    } else {
        panic!("unsupported host architecture for Omega native planning")
    }
}

fn host_object_format() -> ObjectFormat {
    if cfg!(target_os = "macos") {
        ObjectFormat::MachO
    } else if cfg!(target_os = "windows") {
        ObjectFormat::Coff
    } else {
        ObjectFormat::Elf
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NativeTarget, ProgramEntryPhysicalContractPackage, ProgramEntrySchema,
        ProgramEntryVisibleParameters, TargetProfile, TargetRequiredRootSlotDeclaration,
    };

    #[test]
    fn hosted_program_entry_slot_hides_physical_storage_roots() {
        let slot = TargetProfile::WindowsX64.program_entry_slot();
        assert_eq!(slot.owner, TargetProfile::WindowsX64);
        assert_eq!(slot.slot_name, "ProgramEntry");
        assert_eq!(slot.schema, ProgramEntrySchema::HostedApplication);
        assert_eq!(
            slot.semantic_arrival_requirement,
            "ProgramStorageEntry::enter"
        );
        assert_eq!(slot.boundary_schema, Some("WindowsX86_64Application"));
        assert_eq!(
            slot.physical_arrival_requirement,
            Some("WindowsProcessEntry::enter")
        );
        assert_eq!(
            slot.physical_contract_package,
            Some(ProgramEntryPhysicalContractPackage::WindowsX64)
        );
        let physical_package = slot
            .physical_contract_package
            .expect("Windows x86-64 must select its closed physical-contract package");
        assert_eq!(
            physical_package.manifest_identity(),
            "omega::language::std::targets::windows_x86_64::entry"
        );
        assert_eq!(
            physical_package.package_relative_source(),
            "targets/windows_x86_64/entry.omg"
        );
        assert_eq!(physical_package.contract_name(), "Windows x86-64");
        assert_eq!(
            slot.physical_calling_convention,
            Some(super::ProgramEntryCallingConvention::MicrosoftX64)
        );
        assert_eq!(
            slot.semantic_calling_convention,
            Some(super::ProgramEntryCallingConvention::MicrosoftX64)
        );
        assert_eq!(slot.visible_parameters, ProgramEntryVisibleParameters::None);
    }

    #[test]
    fn uefi_program_entry_slot_exposes_exact_storage_root_shape() {
        let slot = TargetProfile::UefiX64.program_entry_slot();
        assert_eq!(slot.schema, ProgramEntrySchema::ProgramStorageApplication);
        assert_eq!(slot.boundary_schema, Some("UefiApplication"));
        assert_eq!(
            slot.physical_arrival_requirement,
            Some("UefiPhysicalEntry::enter")
        );
        assert_eq!(
            slot.physical_contract_package,
            Some(ProgramEntryPhysicalContractPackage::UefiX64)
        );
        let physical_package = slot
            .physical_contract_package
            .expect("UEFI must select its closed physical-contract package");
        assert_eq!(
            physical_package.manifest_identity(),
            "omega::language::std::targets::uefi_x86_64::entry"
        );
        assert_eq!(
            physical_package.package_relative_source(),
            "targets/uefi_x86_64/entry.omg"
        );
        assert_eq!(
            slot.physical_calling_convention,
            Some(super::ProgramEntryCallingConvention::MicrosoftX64)
        );
        assert_eq!(
            slot.semantic_calling_convention,
            Some(super::ProgramEntryCallingConvention::MicrosoftX64)
        );
        assert_eq!(
            slot.visible_parameters,
            ProgramEntryVisibleParameters::ImageAndInitialStorage
        );
    }

    #[test]
    fn legacy_cli_aliases_normalize_to_canonical_profile_identities() {
        for (legacy, canonical, profile, identity) in [
            (
                "linux_x64",
                "linux_x86_64",
                TargetProfile::LinuxX64,
                "omega.target-profile.v1:linux_x86_64",
            ),
            (
                "windows_x64",
                "windows_x86_64",
                TargetProfile::WindowsX64,
                "omega.target-profile.v1:windows_x86_64",
            ),
            (
                "uefi_x64",
                "uefi_x86_64",
                TargetProfile::UefiX64,
                "omega.target-profile.v1:uefi_x86_64",
            ),
        ] {
            assert_eq!(
                TargetProfile::from_omega_target_name(Some(legacy)).unwrap(),
                profile
            );
            assert_eq!(
                TargetProfile::from_omega_target_name(Some(canonical)).unwrap(),
                profile
            );
            assert_eq!(profile.target_name(), canonical);
            assert_eq!(profile.identity().as_str(), identity);
            assert_eq!(
                TargetProfile::from_root_slot_owner(canonical).unwrap(),
                profile
            );
            assert!(TargetProfile::from_root_slot_owner(legacy).is_err());
        }
    }

    #[test]
    fn required_root_catalog_is_complete_ordered_and_target_owned() {
        for profile in TargetProfile::ALL {
            let slots = profile.required_root_slots().collect::<Vec<_>>();
            assert_eq!(slots.len(), 1);
            let TargetRequiredRootSlotDeclaration::ProgramEntry(slot) = slots[0];
            assert_eq!(slot, profile.program_entry_slot());
            assert_eq!(slot.owner, profile);
            assert_eq!(
                profile.required_root_slot("ProgramEntry"),
                Some(slot.into())
            );
            assert_eq!(profile.required_root_slot("NotDeclared"), None);
        }
    }

    #[test]
    fn build_target_cases_round_trip_every_exact_profile() {
        for profile in TargetProfile::ALL {
            assert_eq!(
                TargetProfile::from_build_case_name(profile.build_case_name()),
                Some(profile)
            );
        }
        assert_eq!(TargetProfile::from_build_case_name("Host"), None);
        assert_eq!(TargetProfile::from_build_case_name("WindowsX64"), None);
    }

    #[test]
    fn alpha_bootstrap_is_an_ordinary_recognized_profile() {
        let profile = TargetProfile::AlphaBootstrap;

        assert_eq!(
            TargetProfile::from_root_slot_owner("alpha_bootstrap").unwrap(),
            profile
        );
        assert_eq!(
            TargetProfile::from_omega_target_name(Some("alpha_bootstrap")).unwrap(),
            profile
        );
        assert_eq!(
            TargetProfile::from_canonical_target_name("alpha_bootstrap").unwrap(),
            profile
        );
        assert_eq!(profile.target_name(), "alpha_bootstrap");
        assert_eq!(profile.root_slot_owner_name(), "alpha_bootstrap");
        assert_eq!(
            profile.identity().as_str(),
            "omega.target-profile.v1:alpha_bootstrap"
        );
        assert_eq!(profile.build_case_name(), "AlphaBootstrap");

        // The slot schema comes from the same catch-all row the hosted
        // compatibility profiles use: the source binding supplies only the
        // machine, and no toolchain physical-contract package applies.
        let slot = profile.program_entry_slot();
        assert_eq!(slot.owner, profile);
        assert_eq!(slot.slot_name, "ProgramEntry");
        assert_eq!(slot.schema, ProgramEntrySchema::HostedApplication);
        assert_eq!(slot.visible_parameters, ProgramEntryVisibleParameters::None);
        assert_eq!(slot.physical_contract_package, None);
        assert_eq!(
            slot.semantic_arrival_requirement,
            "ProgramStorageEntry::enter"
        );
        assert_eq!(
            profile
                .required_root_slot("ProgramEntry")
                .map(|slot| slot.slot_name()),
            Some("ProgramEntry")
        );
    }

    #[test]
    fn alpha_bootstrap_selection_reports_not_implemented() {
        let diagnostic = NativeTarget::from_omega_target_name(Some("alpha_bootstrap")).unwrap_err();
        assert_eq!(
            diagnostic.to_string(),
            "error: native realization for target profile `alpha_bootstrap` is not implemented"
        );

        // Unknown owners and misspellings still reject rather than silently
        // becoming an inactive recognized profile.
        for unknown in [
            "alpha",
            "alpha-bootstrap",
            "alpha_bootstrap_",
            "alpha_bootstrap::ProgramEntry",
        ] {
            assert!(
                TargetProfile::from_root_slot_owner(unknown).is_err(),
                "{unknown} must stay an unknown target profile"
            );
            assert!(
                TargetProfile::from_omega_target_name(Some(unknown)).is_err(),
                "{unknown} must stay an unknown target profile"
            );
        }
    }
}
