use psi_diagnostics::Diagnostic;

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

/// Target-owned declaration of the first environment-to-program root slot.
/// The source binding supplies only `machine`; every other field belongs to
/// the selected target profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntrySlotDeclaration {
    pub owner: TargetProfile,
    pub slot_name: &'static str,
    pub schema: ProgramEntrySchema,
    pub arrival_requirement: &'static str,
    /// Source boundary schema whose evaluated Calling<C> plan owns the
    /// physical entry contract. `None` marks a profile not yet migrated from
    /// its hosted compatibility bridge.
    pub boundary_schema: Option<&'static str>,
    pub calling_convention: Option<ProgramEntryCallingConvention>,
    pub visible_parameters: ProgramEntryVisibleParameters,
    pub receiver: ProgramEntryReceiverProvisioning,
}

impl TargetProfile {
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
        let (schema, visible_parameters, boundary_schema, calling_convention) = match self {
            Self::UefiX64 => (
                ProgramEntrySchema::ProgramStorageApplication,
                ProgramEntryVisibleParameters::ImageAndInitialStorage,
                Some("UefiApplication"),
                Some(ProgramEntryCallingConvention::MicrosoftX64),
            ),
            _ => (
                ProgramEntrySchema::HostedApplication,
                ProgramEntryVisibleParameters::None,
                None,
                None,
            ),
        };
        ProgramEntrySlotDeclaration {
            owner: self,
            slot_name: "ProgramEntry",
            schema,
            arrival_requirement: "ProgramStorageEntry::enter",
            boundary_schema,
            calling_convention,
            visible_parameters,
            receiver: ProgramEntryReceiverProvisioning::NoneOrProvisionedZii,
        }
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
    use super::{ProgramEntrySchema, ProgramEntryVisibleParameters, TargetProfile};

    #[test]
    fn hosted_program_entry_slot_hides_physical_storage_roots() {
        let slot = TargetProfile::WindowsX64.program_entry_slot();
        assert_eq!(slot.owner, TargetProfile::WindowsX64);
        assert_eq!(slot.slot_name, "ProgramEntry");
        assert_eq!(slot.schema, ProgramEntrySchema::HostedApplication);
        assert_eq!(slot.arrival_requirement, "ProgramStorageEntry::enter");
        assert_eq!(slot.boundary_schema, None);
        assert_eq!(slot.calling_convention, None);
        assert_eq!(slot.visible_parameters, ProgramEntryVisibleParameters::None);
    }

    #[test]
    fn uefi_program_entry_slot_exposes_exact_storage_root_shape() {
        let slot = TargetProfile::UefiX64.program_entry_slot();
        assert_eq!(slot.schema, ProgramEntrySchema::ProgramStorageApplication);
        assert_eq!(slot.boundary_schema, Some("UefiApplication"));
        assert_eq!(
            slot.calling_convention,
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
}
