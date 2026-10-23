//! Stable target identity retained by representation demand evidence.

mod validation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewRepresentationTargetProfile {
    LinuxArm64,
    LinuxX64,
    MacosArm64,
    WindowsX64,
    UefiX64,
    CrossPlatformCli,
    LocalUnchecked,
    MacosX64,
}

impl PackageReviewRepresentationTargetProfile {
    /// The backend target profile this review profile names. One table serves
    /// every consumer: representation validation checks the recorded geometry
    /// against it, the baseline checks that its components agree on one
    /// target, and provider selection checks a calling application's target.
    pub(crate) fn backend_profile(self) -> target::TargetProfile {
        match self {
            Self::LinuxArm64 => target::TargetProfile::LinuxArm64,
            Self::LinuxX64 => target::TargetProfile::LinuxX64,
            Self::MacosArm64 => target::TargetProfile::MacosArm64,
            Self::MacosX64 => target::TargetProfile::MacosX64,
            Self::WindowsX64 => target::TargetProfile::WindowsX64,
            Self::UefiX64 => target::TargetProfile::UefiX64,
            Self::CrossPlatformCli => target::TargetProfile::CrossPlatformCli,
            Self::LocalUnchecked => target::TargetProfile::LocalUnchecked,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewRepresentationArchitecture {
    Aarch64,
    X86_64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewRepresentationObjectFormat {
    Elf,
    MachO,
    Coff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewRepresentationTarget {
    pub(crate) profile: PackageReviewRepresentationTargetProfile,
    pub(crate) architecture: PackageReviewRepresentationArchitecture,
    pub(crate) object_format: PackageReviewRepresentationObjectFormat,
    pub(crate) pointer_size: u16,
    pub(crate) pointer_alignment: u16,
}

impl PackageReviewRepresentationTarget {
    pub const fn profile(self) -> PackageReviewRepresentationTargetProfile {
        self.profile
    }

    pub const fn architecture(self) -> PackageReviewRepresentationArchitecture {
        self.architecture
    }

    pub const fn object_format(self) -> PackageReviewRepresentationObjectFormat {
        self.object_format
    }

    pub const fn pointer_size(self) -> u16 {
        self.pointer_size
    }

    pub const fn pointer_alignment(self) -> u16 {
        self.pointer_alignment
    }
}
