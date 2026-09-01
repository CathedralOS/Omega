//! Stable target identity retained by representation demand evidence.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewRepresentationTargetProfile {
    LinuxArm64,
    LinuxX64,
    MacosArm64,
    WindowsX64,
    UefiX64,
    CrossPlatformCli,
    LocalUnchecked,
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
