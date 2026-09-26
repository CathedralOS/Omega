//! Every way a build file can fail projection, with its user-facing text.

use crate::BuildDeclarationKind;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDeclarationError {
    MissingBuildFile { path: PathBuf },
    ReadBuildFile { path: PathBuf, message: String },
    InvalidBuildFileEncoding { path: PathBuf },
    Lex { message: String },
    Parse { message: String },
    AuthoredToolchainVocabulary { name: String },
    ScopedBuildMachine { scope: String },
    DuplicateBuildMachines { count: usize },
    InvalidBuildMachine,
    MissingBuildEntry,
    InvalidBuildParameter,
    UnsupportedPackageShape,
    WrongPackageReceiver,
    WrongPackageArguments,
    UnsupportedApplicationShape,
    WrongApplicationReceiver,
    WrongApplicationArguments,
    UnsupportedMemberShape,
    WrongMemberReceiver,
    WrongMemberArguments,
    UnsupportedArtifactOnlyShape,
    WrongArtifactOnlyReceiver,
    WrongArtifactOnlyArguments,
    DuplicateArtifactOnlyDeclarations { count: usize },
    ArtifactOnlyRequiresApplication { found: BuildDeclarationKind },
    MissingBuildDeclaration,
    MissingPackageDeclaration,
    ExpectedPackageDeclaration { found: BuildDeclarationKind },
    DuplicatePackageDeclarations { count: usize },
    DuplicateApplicationDeclarations { count: usize },
    MixedBuildDeclarations,
    NameNotStringLiteral,
    NameNotUtf8,
    InvalidPackageName { message: String },
    MemberPathNotStringLiteral,
    MemberPathNotUtf8,
    InvalidWorkspaceMemberPath,
    DuplicateWorkspaceMember { path: String },
}

impl fmt::Display for BuildDeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBuildFile { path } => {
                write!(formatter, "package build file is missing: {}", path.display())
            }
            Self::ReadBuildFile { path, message } => {
                write!(formatter, "cannot read {}: {message}", path.display())
            }
            Self::InvalidBuildFileEncoding { path } => {
                write!(formatter, "{} is not UTF-8 Omega source", path.display())
            }
            Self::Lex { message } => write!(formatter, "cannot lex package build: {message}"),
            Self::Parse { message } => write!(formatter, "cannot parse package build: {message}"),
            Self::AuthoredToolchainVocabulary { name } => write!(
                formatter,
                "package build must not declare toolchain package vocabulary `{name}`"
            ),
            Self::ScopedBuildMachine { scope } => {
                write!(
                    formatter,
                    "selected build.omg must declare the free `machine build(builder: &mut Build)` entry; `{scope}::build` is an ordinary scoped machine and cannot be selected"
                )
            }
            Self::DuplicateBuildMachines { count } => {
                write!(formatter, "package build declares `build` {count} times")
            }
            Self::InvalidBuildMachine => formatter.write_str(
                "package build must be a bodyful, unscoped, nontarget, nongeneric ordinary machine",
            ),
            Self::MissingBuildEntry => {
                formatter.write_str("package build machine has no callable entry")
            }
            Self::InvalidBuildParameter => formatter.write_str(
                "package build machine must have exactly one parameter: `builder: &mut Build`",
            ),
            Self::UnsupportedPackageShape => formatter.write_str(
                "package declaration must be one direct canonical `builder.package(\"kebab_name\")` statement in the root build entry",
            ),
            Self::WrongPackageReceiver => formatter.write_str(
                "package declaration receiver must be the root build machine's first parameter",
            ),
            Self::WrongPackageArguments => formatter.write_str(
                "`builder.package` must have one direct name literal and accepts no static, evidence, operational, or discard modifiers",
            ),
            Self::UnsupportedApplicationShape => formatter.write_str(
                "application declaration must be one direct canonical `builder.application(\"kebab_name\")` statement in the root build entry",
            ),
            Self::WrongApplicationReceiver => formatter.write_str(
                "application declaration receiver must be the root build machine's first parameter",
            ),
            Self::WrongApplicationArguments => formatter.write_str(
                "`builder.application` must have one direct name literal and accepts no static, evidence, operational, or discard modifiers",
            ),
            Self::UnsupportedMemberShape => formatter.write_str(
                "workspace members must be direct canonical `builder.member(\"relative/path\")` statements in the root build entry",
            ),
            Self::WrongMemberReceiver => formatter.write_str(
                "workspace member declaration receiver must be the root build machine's first parameter",
            ),
            Self::WrongMemberArguments => formatter.write_str(
                "`builder.member` must have one direct path literal and accepts no static, evidence, operational, or discard modifiers",
            ),
            Self::UnsupportedArtifactOnlyShape => formatter.write_str(
                "artifact-only selection must be one direct canonical `builder.artifact_only()` statement in the root build entry",
            ),
            Self::WrongArtifactOnlyReceiver => formatter.write_str(
                "artifact-only selection receiver must be the root build machine's first parameter",
            ),
            Self::WrongArtifactOnlyArguments => formatter.write_str(
                "`builder.artifact_only` takes no arguments and accepts no static, evidence, operational, or discard modifiers",
            ),
            Self::DuplicateArtifactOnlyDeclarations { count } => write!(
                formatter,
                "application build selects artifact-only mode {count} times"
            ),
            Self::ArtifactOnlyRequiresApplication { found } => write!(
                formatter,
                "`builder.artifact_only()` modifies an application declaration; this build declares {}",
                found.as_str()
            ),
            Self::MissingBuildDeclaration => formatter.write_str(
                "build must declare exactly one kind through `builder.package`, `builder.application`, or one or more `builder.member` statements",
            ),
            Self::MissingPackageDeclaration => formatter.write_str(
                "package build must contain one direct `builder.package(\"kebab_name\")` declaration",
            ),
            Self::ExpectedPackageDeclaration { found } => write!(
                formatter,
                "expected a package build declaration, found an explicit {} declaration",
                found.as_str()
            ),
            Self::DuplicatePackageDeclarations { count } => {
                write!(formatter, "package build declares its package name {count} times")
            }
            Self::DuplicateApplicationDeclarations { count } => write!(
                formatter,
                "application build declares its application name {count} times"
            ),
            Self::MixedBuildDeclarations => formatter.write_str(
                "build must declare exactly one kind; package, application, and workspace member declarations cannot be mixed",
            ),
            Self::NameNotStringLiteral => {
                formatter.write_str("package or application name must be a direct string literal")
            }
            Self::NameNotUtf8 => {
                formatter.write_str("package or application name must contain UTF-8 bytes")
            }
            Self::InvalidPackageName { message } => formatter.write_str(message),
            Self::MemberPathNotStringLiteral => {
                formatter.write_str("workspace member path must be a direct string literal")
            }
            Self::MemberPathNotUtf8 => {
                formatter.write_str("workspace member path must contain UTF-8 bytes")
            }
            Self::InvalidWorkspaceMemberPath => formatter.write_str(
                "workspace member path must be a canonical portable relative path without `.` or `..` components",
            ),
            Self::DuplicateWorkspaceMember { path } => {
                write!(formatter, "workspace member path `{path}` is declared more than once")
            }
        }
    }
}

impl std::error::Error for BuildDeclarationError {}
