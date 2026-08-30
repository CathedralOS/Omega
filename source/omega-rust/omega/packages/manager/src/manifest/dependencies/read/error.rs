use crate::manifest::roles::BuildDeclarationError;
use psi_source::SourceSpan;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyProjectionError {
    MissingBuildFile {
        path: PathBuf,
    },
    ReadBuildFile {
        path: PathBuf,
        message: String,
    },
    InvalidBuildFileEncoding {
        path: PathBuf,
    },
    Lex {
        message: String,
    },
    Parse {
        message: String,
    },
    BuildDeclaration(Box<BuildDeclarationError>),
    AuthoredToolchainVocabulary {
        name: String,
    },
    ScopedBuildMachine {
        scope: String,
    },
    DuplicateBuildMachines {
        count: usize,
    },
    InvalidBuildMachine,
    MissingBuildEntry,
    InvalidBuildParameter,
    UnsupportedDependencyShape,
    WrongDependencyReceiver,
    WrongDependencyArguments,
    UnsupportedStateTransition {
        state: String,
        transition: SourceSpan,
    },
    UnknownTargetProfile {
        case_name: String,
        arm: SourceSpan,
    },
    UnreachableDependency {
        state: String,
        dependency: SourceSpan,
    },
    TaintedDependencyPath {
        state: String,
        dependency: SourceSpan,
        provenance: DependencyPathProvenance,
    },
    MixedDependencyPaths {
        state: String,
        dependency: SourceSpan,
        provenance: DependencyPathProvenance,
    },
    TargetConditionedResolutionUnavailable,
    AliasNotString,
    AliasNotUtf8,
    InvalidAlias {
        alias: String,
    },
    SourceNotLiteral,
    WrongSourceType,
    MissingSourceCase,
    UnsupportedSourceCase {
        case_name: String,
    },
    WrongSourceFields {
        case_name: String,
    },
    SourceFieldNotString {
        field: String,
    },
    SourceFieldNotUtf8 {
        field: String,
    },
    SelectionNotLiteral,
    WrongSelectionType,
    MissingSelectionCase,
    UnsupportedSelectionCase {
        case_name: String,
    },
    WrongSelectionFields {
        case_name: String,
    },
    InvalidSelectedPackage {
        package: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyPathTaint {
    WildcardTargetArm,
    RuntimeSubjectTransition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyPathProvenance {
    pub state: String,
    pub transition: SourceSpan,
    pub taint: DependencyPathTaint,
}

impl fmt::Display for DependencyProjectionError {
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
            Self::BuildDeclaration(error) => {
                write!(formatter, "cannot project dependencies without a valid project role: {error}")
            }
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
            Self::InvalidBuildParameter => formatter.write_str(
                "package build machine's first parameter must be `builder: &mut Build`",
            ),
            Self::UnsupportedDependencyShape => formatter.write_str(
                "dependencies are statically projected before build execution and must be direct canonical `builder.depend(Source::...)` or `builder.depend_as(alias, Source::...)` statements in the root build entry; arbitrary build control flow is not evaluated, and target-conditioned edges require an explicit static dependency-condition surface",
            ),
            Self::WrongDependencyReceiver => formatter.write_str(
                "dependency request receiver must be the root build machine's first parameter",
            ),
            Self::WrongDependencyArguments => formatter.write_str(
                "`builder.depend` must have one source argument and `builder.depend_as` must have one direct alias literal followed by one source argument; neither accepts static, evidence, operational, or discard modifiers",
            ),
            Self::UnsupportedStateTransition { state, transition } => write!(
                formatter,
                "build state `{state}` has a transition to another build state whose argument shape cannot preserve static builder provenance at bytes {}..{}",
                transition.span.start,
                transition.span.end,
            ),
            Self::UnknownTargetProfile { case_name, arm } => write!(
                formatter,
                "target dependency arm names unknown trusted profile `TargetProfile::{case_name}` at bytes {}..{}",
                arm.span.start,
                arm.span.end,
            ),
            Self::UnreachableDependency { state, dependency } => write!(
                formatter,
                "dependency in build state `{state}` at bytes {}..{} is unreachable from the build entry; remove it or connect the state with an unconditional or exact `builder.target` transition",
                dependency.span.start,
                dependency.span.end,
            ),
            Self::TaintedDependencyPath {
                state,
                dependency,
                provenance,
            } => write!(
                formatter,
                "dependency in build state `{state}` at bytes {}..{} is reachable only through {} in state `{}` at bytes {}..{}; hoist a common dependency or use exact `builder.target` arms",
                dependency.span.start,
                dependency.span.end,
                provenance.taint,
                provenance.state,
                provenance.transition.span.start,
                provenance.transition.span.end,
            ),
            Self::MixedDependencyPaths {
                state,
                dependency,
                provenance,
            } => write!(
                formatter,
                "dependency in build state `{state}` at bytes {}..{} has both authorized and tainted paths; the tainted path crosses {} in state `{}` at bytes {}..{} and must be removed",
                dependency.span.start,
                dependency.span.end,
                provenance.taint,
                provenance.state,
                provenance.transition.span.start,
                provenance.transition.span.end,
            ),
            Self::TargetConditionedResolutionUnavailable => formatter.write_str(
                "target-conditioned dependencies were projected successfully, but this resolver entry point has no exact target-profile input and refuses to flatten profile columns",
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
                "dependency source must be a direct `Source::Path` or `Source::Git` literal",
            ),
            Self::WrongSourceType => {
                formatter.write_str("dependency source literal must construct `Source`")
            }
            Self::MissingSourceCase => formatter.write_str(
                "dependency source must select the `Source::Path` or `Source::Git` case",
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
                "selected package `{package}` must use canonical kebab-case spelling"
            ),
        }
    }
}

impl std::error::Error for DependencyProjectionError {}

impl fmt::Display for DependencyPathTaint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WildcardTargetArm => "a wildcard target arm",
            Self::RuntimeSubjectTransition => "a transition on a runtime subject",
        })
    }
}
