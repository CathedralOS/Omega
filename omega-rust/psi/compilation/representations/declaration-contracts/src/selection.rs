use source_text::SourceSpan;

use semantic_vocabulary::{CrashCause, TerminalAuthorityClass};
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredRootGrant {
    /// Authored display text; settlement uses `subject`.
    pub selector: String,
    pub subject: RootGrantSubject,
    pub subject_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub selecting_machine: SymbolHandle,
    pub source_span: source_text::SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootGrantSubject {
    Provider {
        declaration: String,
        application: Option<String>,
    },
    BoundaryRequirement {
        declaration: String,
        callable: String,
    },
    AcceptedMachine {
        declaration: String,
        machine: SymbolHandle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoredBehaviorExclusionKind {
    CrashCause {
        cause: CrashCause,
        case_symbol: SymbolHandle,
    },
    PhysicalAuthorityClass {
        class: TerminalAuthorityClass,
        case_symbol: SymbolHandle,
    },
    Service {
        identity: String,
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredBehaviorExclusion {
    pub kind: AuthoredBehaviorExclusionKind,
    pub selecting_machine: SymbolHandle,
    pub source_span: source_text::SourceSpan,
}

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompositionMode {
    #[default]
    Fused,
    Independent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingProducer {
    pub package: semantic_vocabulary::PackageKeyIdentity,
    pub declaration_path: String,
    pub callable_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumedBindingContract {
    pub required: SymbolHandle,
    pub supplied: SymbolHandle,
    pub selecting_machine: SymbolHandle,
    pub source_span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
    pub implementation_symbol: SymbolHandle,
    pub source_span: Option<SourceSpan>,
}
