//! Target keys in evaluated Build rows are ordinary catalog values.
//!
//! This vocabulary carries no ISA or host authority. Omega rejoins each key to
//! its deployment catalog when realizing the selected provider closure. The
//! checked prelude encodes these values; engines reject unknown protocol codes.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProductTarget {
    LinuxArm64,
    LinuxX86_64,
    MacosArm64,
    MacosX86_64,
    WindowsX86_64,
    UefiX86_64,
    CrossPlatformCli,
    LocalUnchecked,
    AlphaBootstrap,
}

impl ProductTarget {
    pub const ALL: [Self; 9] = [
        Self::LinuxArm64,
        Self::LinuxX86_64,
        Self::MacosArm64,
        Self::MacosX86_64,
        Self::WindowsX86_64,
        Self::UefiX86_64,
        Self::CrossPlatformCli,
        Self::LocalUnchecked,
        Self::AlphaBootstrap,
    ];

    pub fn from_code(code: u64) -> Option<Self> {
        usize::try_from(code)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
    }

    pub fn from_canonical_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|target| target.canonical_name() == name)
    }

    pub fn code(self) -> u64 {
        Self::ALL
            .into_iter()
            .position(|target| target == self)
            .expect("complete target catalog") as u64
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::LinuxArm64 => "linux_arm64",
            Self::LinuxX86_64 => "linux_x86_64",
            Self::MacosArm64 => "macos_arm64",
            Self::MacosX86_64 => "macos_x86_64",
            Self::WindowsX86_64 => "windows_x86_64",
            Self::UefiX86_64 => "uefi_x86_64",
            Self::CrossPlatformCli => "cross_platform_cli",
            Self::LocalUnchecked => "local_unchecked",
            Self::AlphaBootstrap => "alpha_bootstrap",
        }
    }

    pub const fn source_case(self) -> &'static str {
        match self {
            Self::LinuxArm64 => "LinuxArm64",
            Self::LinuxX86_64 => "LinuxX86_64",
            Self::MacosArm64 => "MacosArm64",
            Self::MacosX86_64 => "MacosX86_64",
            Self::WindowsX86_64 => "WindowsX86_64",
            Self::UefiX86_64 => "UefiX86_64",
            Self::CrossPlatformCli => "CrossPlatformCli",
            Self::LocalUnchecked => "LocalUnchecked",
            Self::AlphaBootstrap => "AlphaBootstrap",
        }
    }
}
