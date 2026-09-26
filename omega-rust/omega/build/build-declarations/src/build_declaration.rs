//! The build declaration: the authoritative project role a `build.omg`
//! declares, the package, application and workspace forms it takes, and the
//! validated names and member paths inside them.

use crate::syntax_projection::{is_portable_path_byte, is_snake_case};
use std::fmt;

pub const BUILD_FILE_NAME: &str = "build.omg";

pub(crate) const BUILD_MACHINE_NAME: &str = "build";

pub(crate) const BUILD_TYPE_NAME: &str = "Build";

pub(crate) const BUILDER_PARAMETER_NAME: &str = "builder";

pub(crate) const PACKAGE_MACHINE_NAME: &str = "package";

pub(crate) const APPLICATION_MACHINE_NAME: &str = "application";

pub(crate) const MEMBER_MACHINE_NAME: &str = "member";

pub(crate) const ARTIFACT_ONLY_MACHINE_NAME: &str = "artifact_only";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectName(String);

impl ProjectName {
    /// Validate a borrowed spelling without constructing an owned diagnostic.
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceMemberPath(String);

impl WorkspaceMemberPath {
    pub fn parse(value: impl Into<String>) -> Result<Self, InvalidWorkspaceMemberPath> {
        let value = value.into();
        if value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
            || value.bytes().any(|byte| byte.is_ascii_control())
            || value.split('/').any(|component| {
                component.is_empty()
                    || matches!(component, "." | "..")
                    || !component.bytes().all(is_portable_path_byte)
            })
        {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildDeclarationKind {
    Package,
    Application,
    Workspace,
}

impl BuildDeclarationKind {
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
    /// One direct unconditional `builder.artifact_only()` statement in the
    /// root build entry selected this application modifier. Artifact-only is
    /// an application flag, never a fourth project role: executable output
    /// remains the default until the modifier appears exactly once.
    pub artifact_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDeclaration {
    pub members: Vec<WorkspaceMemberPath>,
}
