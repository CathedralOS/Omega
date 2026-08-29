use psi_diagnostics::Diagnostic;

mod elf_loader;
mod foreign_locator;
mod uefi_boot_services;
mod uefi_boot_services_occurrence;
mod uefi_system_table;
mod uefi_system_table_occurrence;

pub use elf_loader::{
    ElfInterpreterPlanValidationError, NormalizedElfInterpreterPlan, normalize_elf_interpreter_plan,
};
pub use foreign_locator::{
    ForeignLocatorCandidate, ForeignLocatorValidationError, NormalizedForeignLocator,
    normalize_foreign_locator,
};
pub use uefi_boot_services::{
    UEFI_LOADED_IMAGE_PROTOCOL_GUID, UefiBootServicesNativeField, UefiBootServicesNativeFieldKind,
    UefiBootServicesNativeFieldLayout, UefiBootServicesNativeLayoutError, UefiProtocolGuid,
    ValidatedUefiBootServicesNativeLayout, plan_uefi_boot_services_native_layout,
};
pub use uefi_boot_services_occurrence::{
    UEFI_BOOT_SERVICES_SIGNATURE, UefiBootServicesOccurrenceValidationError,
    ValidatedUefiBootServicesHeaderIntegrity, validate_uefi_boot_services_occurrence,
};
pub use uefi_system_table::{
    UefiSystemTableNativeField, UefiSystemTableNativeFieldKind, UefiSystemTableNativeFieldLayout,
    UefiSystemTableNativeLayoutError, ValidatedUefiSystemTableNativeLayout,
    plan_uefi_system_table_native_layout,
};
pub use uefi_system_table_occurrence::{
    UEFI_SYSTEM_TABLE_SIGNATURE, UefiSystemTableOccurrenceValidationError,
    ValidatedUefiSystemTableHeaderIntegrity, validate_uefi_system_table_occurrence,
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
}

/// Closed identity of a toolchain-owned physical-entry contract package.
/// Source paths prove membership in this identity; they never define it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntryPhysicalContractPackage {
    UefiX64,
}

impl ProgramEntryPhysicalContractPackage {
    pub const fn manifest_identity(self) -> &'static str {
        match self {
            Self::UefiX64 => "omega::language::std::targets::uefi_x64::entry",
        }
    }

    pub const fn package_relative_source(self) -> &'static str {
        match self {
            Self::UefiX64 => "targets/uefi_x64/entry.omg",
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
    pub const ALL: [Self; 7] = [
        Self::LinuxArm64,
        Self::LinuxX64,
        Self::MacosArm64,
        Self::WindowsX64,
        Self::UefiX64,
        Self::CrossPlatformCli,
        Self::LocalUnchecked,
    ];

    pub const fn identity(self) -> TargetProfileIdentity {
        TargetProfileIdentity(match self {
            Self::LinuxArm64 => "omega.target-profile.v1:linux_arm64",
            Self::LinuxX64 => "omega.target-profile.v1:linux_x64",
            Self::MacosArm64 => "omega.target-profile.v1:macos_arm64",
            Self::WindowsX64 => "omega.target-profile.v1:windows_x64",
            Self::UefiX64 => "omega.target-profile.v1:uefi_x64",
            Self::CrossPlatformCli => "omega.target-profile.v1:cross_platform_cli",
            Self::LocalUnchecked => "omega.target-profile.v1:local_unchecked",
        })
    }

    pub fn host() -> Self {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Self::MacosArm64
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Self::LinuxArm64
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Self::LinuxX64
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Self::WindowsX64
        } else {
            panic!("unsupported host profile for Omega native planning")
        }
    }

    pub fn from_omega_target_name(target_name: Option<&str>) -> Result<Self, Diagnostic> {
        match target_name {
            Some("linux_arm64") => Ok(Self::LinuxArm64),
            Some("linux_x64") => Ok(Self::LinuxX64),
            Some("macos_arm64") => Ok(Self::MacosArm64),
            Some("windows_x64") => Ok(Self::WindowsX64),
            Some("uefi_x64") => Ok(Self::UefiX64),
            Some("cross_platform_cli") => Ok(Self::CrossPlatformCli),
            Some("local_unchecked") => Ok(Self::LocalUnchecked),
            None => Ok(Self::host()),
            Some(target_name) => Err(Diagnostic::error(format!(
                "unknown target profile `{target_name}`; expected linux_arm64, linux_x64, macos_arm64, windows_x64, uefi_x64, cross_platform_cli, or local_unchecked"
            ))),
        }
    }

    /// Parse the canonical source-level owner of a target-declared slot. This
    /// namespace is intentionally distinct from deployment CLI target names.
    pub fn from_root_slot_owner(owner: &str) -> Result<Self, Diagnostic> {
        match owner {
            "linux_arm64" => Ok(Self::LinuxArm64),
            "linux_x86_64" => Ok(Self::LinuxX64),
            "macos_arm64" => Ok(Self::MacosArm64),
            "windows_x86_64" => Ok(Self::WindowsX64),
            "uefi_x86_64" => Ok(Self::UefiX64),
            "cross_platform_cli" => Ok(Self::CrossPlatformCli),
            "local_unchecked" => Ok(Self::LocalUnchecked),
            _ => Err(Diagnostic::error(format!(
                "unknown target root-slot owner `{owner}`"
            ))),
        }
    }

    pub const fn target_name(self) -> &'static str {
        match self {
            Self::LinuxArm64 => "linux_arm64",
            Self::LinuxX64 => "linux_x64",
            Self::MacosArm64 => "macos_arm64",
            Self::WindowsX64 => "windows_x64",
            Self::UefiX64 => "uefi_x64",
            Self::CrossPlatformCli => "cross_platform_cli",
            Self::LocalUnchecked => "local_unchecked",
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
            Some(target_name) => TargetProfile::from_omega_target_name(Some(target_name))
                .map(TargetProfile::native_target),
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
        ProgramEntryPhysicalContractPackage, ProgramEntrySchema, ProgramEntryVisibleParameters,
        TargetProfile, TargetRequiredRootSlotDeclaration,
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
        assert_eq!(slot.boundary_schema, None);
        assert_eq!(slot.physical_calling_convention, None);
        assert_eq!(slot.physical_contract_package, None);
        assert_eq!(slot.semantic_calling_convention, None);
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
            "omega::language::std::targets::uefi_x64::entry"
        );
        assert_eq!(
            physical_package.package_relative_source(),
            "targets/uefi_x64/entry.omg"
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
    fn deployment_names_and_root_slot_owners_are_distinct_canonical_namespaces() {
        assert!(TargetProfile::from_omega_target_name(Some("windows_x64")).is_ok());
        assert!(TargetProfile::from_omega_target_name(Some("windows_x86_64")).is_err());
        assert!(TargetProfile::from_root_slot_owner("windows_x86_64").is_ok());
        assert!(TargetProfile::from_root_slot_owner("windows_x64").is_err());
        assert_eq!(
            TargetProfile::WindowsX64.root_slot_owner_name(),
            "windows_x86_64"
        );
    }

    #[test]
    fn required_root_catalog_is_complete_ordered_and_target_owned() {
        for profile in [
            TargetProfile::LinuxArm64,
            TargetProfile::LinuxX64,
            TargetProfile::MacosArm64,
            TargetProfile::WindowsX64,
            TargetProfile::UefiX64,
            TargetProfile::CrossPlatformCli,
            TargetProfile::LocalUnchecked,
        ] {
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
        for profile in [
            TargetProfile::LinuxArm64,
            TargetProfile::LinuxX64,
            TargetProfile::MacosArm64,
            TargetProfile::WindowsX64,
            TargetProfile::UefiX64,
            TargetProfile::CrossPlatformCli,
            TargetProfile::LocalUnchecked,
        ] {
            assert_eq!(
                TargetProfile::from_build_case_name(profile.build_case_name()),
                Some(profile)
            );
        }
        assert_eq!(TargetProfile::from_build_case_name("Host"), None);
        assert_eq!(TargetProfile::from_build_case_name("WindowsX64"), None);
    }
}
