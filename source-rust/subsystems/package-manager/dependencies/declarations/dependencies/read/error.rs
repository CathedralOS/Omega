// Error diagnostics for conservative `build.omg` dependency row projection.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyProjectionError {
    Lex { message: String },
    Parse { message: String },
    AuthoredToolchainVocabulary { name: String },
    ScopedBuildMachine { scope: String },
    DuplicateBuildMachines { count: usize },
    InvalidBuildMachine,
    MissingBuildMachine,
    MissingBuildEntry,
    InvalidBuildParameter,
    UnsupportedDependencyShape,
    WrongDependencyReceiver,
    WrongDependencyArguments,
    AliasNotString,
    AliasNotUtf8,
    InvalidAlias { alias: String },
    SourceNotLiteral,
    WrongSourceType,
    MissingSourceCase,
    UnsupportedSourceCase { case_name: String },
    WrongSourceFields { case_name: String },
    SourceFieldNotString { field: String },
    SourceFieldNotUtf8 { field: String },
    SelectionNotLiteral,
    WrongSelectionType,
    MissingSelectionCase,
    UnsupportedSelectionCase { case_name: String },
    WrongSelectionFields { case_name: String },
    InvalidSelectedPackage { package: String },
}

impl fmt::Display for DependencyProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lex { message } => write!(formatter, "cannot lex package build: {message}"),
            Self::Parse { message } => write!(formatter, "cannot parse package build: {message}"),
            Self::MissingBuildMachine => formatter.write_str("no free Build entry to edit"),
            Self::AuthoredToolchainVocabulary { name } => write!(
                formatter,
                "package build must not declare toolchain dependency vocabulary `{name}`"
            ),
            Self::ScopedBuildMachine { scope } => {
                write!(formatter, "package build machine must be free, not `{scope}::build`")
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
            Self::InvalidBuildParameter => write!(
                formatter,
                "package build machine requires one parameter of type `@mutable {}`",
                package_compilation::declarations::BUILD_TYPE_NAME
            ),
            Self::UnsupportedDependencyShape => formatter.write_str(
                "automatic dependency editing requires unconditional root Build registrations of a literal Source or a fresh dependency description. A configured alias must immediately precede registration, and options or computed registrations need a source edit",
            ),
            Self::WrongDependencyReceiver => formatter.write_str(
                "dependency request must use a static Build call with the root build machine's first parameter as its first operand",
            ),
            Self::WrongDependencyArguments => formatter.write_str(
                "an editable product registration takes a literal Source or a fresh dependency description. Build-scope registrations take a description, and staged options need a source edit",
            ),
            Self::AliasNotString => {
                formatter.write_str("dependency alias must be a direct string literal")
            }
            Self::AliasNotUtf8 => {
                formatter.write_str("dependency alias must contain UTF-8 bytes")
            }
            Self::InvalidAlias { alias } => write!(
                formatter,
                "dependency alias `{alias}` must use snake_case Omega identifier spelling"
            ),
            Self::SourceNotLiteral => formatter.write_str(
                "dependency source must be a direct `Source::Bundled`, `Source::Path`, or `Source::Git` literal",
            ),
            Self::WrongSourceType => {
                formatter.write_str("dependency source literal must construct `Source`")
            }
            Self::MissingSourceCase => formatter.write_str(
                "dependency source must select the `Source::Bundled`, `Source::Path`, or `Source::Git` case",
            ),
            Self::UnsupportedSourceCase { case_name } => {
                write!(formatter, "unsupported dependency source case `Source::{case_name}`")
            }
            Self::WrongSourceFields { case_name } => {
                write!(formatter, "`Source::{case_name}` has noncanonical fields")
            }
            Self::SourceFieldNotString { field } => {
                write!(formatter, "dependency source field `{field}` must be a direct string literal")
            }
            Self::SourceFieldNotUtf8 { field } => {
                write!(formatter, "dependency source field `{field}` must contain UTF-8 bytes")
            }
            Self::SelectionNotLiteral => formatter.write_str(
                "Git package selection must be a direct `PackageSelection::Root` or `PackageSelection::Named` literal",
            ),
            Self::WrongSelectionType => {
                formatter.write_str("Git package selection literal must construct `PackageSelection`")
            }
            Self::MissingSelectionCase => formatter.write_str(
                "Git package selection must select `PackageSelection::Root` or `PackageSelection::Named`",
            ),
            Self::UnsupportedSelectionCase { case_name } => write!(
                formatter,
                "unsupported Git package selection case `PackageSelection::{case_name}`"
            ),
            Self::WrongSelectionFields { case_name } => write!(
                formatter,
                "`PackageSelection::{case_name}` has noncanonical fields"
            ),
            Self::InvalidSelectedPackage { package } => write!(
                formatter,
                "selected package `{package}` must use canonical snake_case spelling"
            ),
        }
    }
}

impl std::error::Error for DependencyProjectionError {}
