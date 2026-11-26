// Validated project roles (`Package`, `Application`, `Workspace`) declared by
// `build.omg`, along with canonical `ProjectName` (lowercase snake_case) and
// portable relative `WorkspaceMemberPath` newtypes.
//
// `ApplicationDeclaration::artifact_only` is a boolean modifier on an
// application role (`Build::artifact_only(builder)`), never a fourth project
// role: executable output remains the default until the modifier appears once.

use std::fmt;

pub const BUILD_FILE_NAME: &str = "build.omg";

pub const BUILD_MODULE_NAME: &str = "Build";

pub const BUILD_TYPE_NAME: &str = "BuildPlan";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct ProjectName(String);

impl ProjectName {
    pub fn is_valid(value: &str) -> bool {
        is_snake_case(value)
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if Self::is_valid(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "package identity `{value}` must start with a lowercase letter and use snake_case lowercase words"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct WorkspaceMemberPath(String);

impl WorkspaceMemberPath {
    pub fn is_valid(value: &str) -> bool {
        !(value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
            || value.bytes().any(|byte| byte.is_ascii_control())
            || value.split('/').any(|component| {
                component.is_empty()
                    || matches!(component, "." | "..")
                    || !component.bytes().all(is_portable_path_byte)
            }))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, InvalidWorkspaceMemberPath> {
        let value = value.into();
        if !Self::is_valid(&value) {
            return Err(InvalidWorkspaceMemberPath);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidWorkspaceMemberPath;

impl fmt::Display for InvalidWorkspaceMemberPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "workspace member path must be a canonical portable relative path without `.` or `..` components",
        )
    }
}

impl std::error::Error for InvalidWorkspaceMemberPath {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDeclaration {
    Package(PackageDeclaration),
    Application(ApplicationDeclaration),
    Workspace(WorkspaceDeclaration),
}

impl BuildDeclaration {
    pub const fn kind(&self) -> BuildDeclarationKind {
        match self {
            Self::Package(_) => BuildDeclarationKind::Package,
            Self::Application(_) => BuildDeclarationKind::Application,
            Self::Workspace(_) => BuildDeclarationKind::Workspace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum BuildDeclarationKind {
    Package,
    Application,
    Workspace,
}

impl BuildDeclarationKind {
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::Package => 0,
            Self::Application => 1,
            Self::Workspace => 2,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Package => "package",
            Self::Application => "application",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDeclaration {
    pub name: ProjectName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationDeclaration {
    pub name: ProjectName,
    pub artifact_only: bool, // Set when `Build::artifact_only(builder)` appears once in the root build entry.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDeclaration {
    pub members: Vec<WorkspaceMemberPath>,
}

fn is_snake_case(value: &str) -> bool {
    if !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase) || value.ends_with('_') {
        return false;
    }
    let mut previous_separator = false;
    for byte in value.bytes() {
        if byte == b'_' {
            if previous_separator {
                return false;
            }
            previous_separator = true;
        } else {
            previous_separator = false;
            if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
                return false;
            }
        }
    }
    true
}

fn is_portable_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
}
