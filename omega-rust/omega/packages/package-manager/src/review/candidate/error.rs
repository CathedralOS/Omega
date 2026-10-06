use super::PackageSourceVerificationPhase;
use crate::declarations::PackageKey;
use crate::resolution::source::PackageSourceSelectionEvidenceError;
use diagnostics::Diagnostic;
use filesystem_authority::FilesystemSponsorError;
use package_compilation::{
    AcceptedSemanticBindingRole, BuildDeclarationKind, PackageCompilationInputError,
};
use package_evidence::encoding::PackageReviewEncodingError;
use package_source::SourceResolveError;
use std::fmt;
use std::io;
use std::path::PathBuf;

mod diagnostic_output;

#[derive(Debug)]
pub enum CompileResolvedPackageReviewsError {
    UnsupportedBuildActivation {
        package: PackageKey,
        reason: &'static str,
    },
    BuildStagingCreate {
        path: PathBuf,
        error: io::Error,
    },
    BuildStagingSponsor {
        path: PathBuf,
        error: FilesystemSponsorError,
    },
    BuildStagingCleanup {
        path: PathBuf,
        error: io::Error,
        prior: Option<Box<CompileResolvedPackageReviewsError>>,
    },
    SourceCustody {
        compiling_package: PackageKey,
        source_package: PackageKey,
        phase: PackageSourceVerificationPhase,
        error: SourceResolveError,
    },
    SourceSelectionCustody {
        compiling_package: PackageKey,
        source_package: PackageKey,
        phase: PackageSourceVerificationPhase,
        error: PackageSourceSelectionEvidenceError,
    },
    SemanticBindingConsumerAbsent {
        consumer: PackageKey,
        role: AcceptedSemanticBindingRole,
    },
    SemanticBindingContextMismatch {
        consumer: PackageKey,
        role: AcceptedSemanticBindingRole,
    },
    DuplicateConsumerSemanticBindingRole {
        consumer: PackageKey,
        role: AcceptedSemanticBindingRole,
    },
    AmbiguousCandidateSemanticBinding {
        consumer: PackageKey,
        role: AcceptedSemanticBindingRole,
        candidate_count: usize,
    },
    InvalidCandidateSemanticBinding {
        consumer: PackageKey,
        role: AcceptedSemanticBindingRole,
    },
    CompilationInputs {
        package: PackageKey,
        errors: Vec<PackageCompilationInputError>,
    },
    Compilation {
        package: PackageKey,
        diagnostics: Vec<Diagnostic>,
    },
    Projection {
        package: PackageKey,
        diagnostics: Vec<Diagnostic>,
    },
    Encoding {
        package: PackageKey,
        error: PackageReviewEncodingError,
    },
    SourceConsumptionMissing {
        package: PackageKey,
    },
    SourceConsumptionDrift {
        package: PackageKey,
        diagnostics: Vec<Diagnostic>,
    },
    IdentityMismatch {
        package: PackageKey,
    },
    InvalidProductionRootRole {
        package: PackageKey,
        role: BuildDeclarationKind,
    },
    RetainedPolicyCanonicalBudget {
        package: PackageKey,
        maximum_bytes: usize,
    },
    /// A consumer's `Independent` selection names a dependency occurrence
    /// carried from a persisted record that retains no component description.
    /// The closure pass reruns with that occurrence compiled fresh; this
    /// escapes only if the fresh occurrence still publishes none.
    ComponentDescriptionNotRetained {
        package: PackageKey,
        component: semantic_vocabulary::PackageKeyIdentity,
        purpose: crate::declarations::DependencyPurpose,
    },
}

impl fmt::Display for CompileResolvedPackageReviewsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticBindingContextMismatch { consumer, role } => write!(
                formatter,
                "semantic binding {role:?} for package `{}` names an unrequested checked occurrence",
                consumer.name().as_str()
            ),
            Self::UnsupportedBuildActivation { package, reason } => write!(
                formatter,
                "cannot schedule build activation for package `{}`: {reason}",
                package.name().as_str()
            ),
            Self::BuildStagingCreate { path, error } => write!(
                formatter,
                "could not create a fresh package-review build session at `{}`: {error}",
                path.display()
            ),
            Self::BuildStagingSponsor { path, error } => write!(
                formatter,
                "failed to sponsor package-review staging root `{}`: {error}",
                path.display()
            ),
            Self::BuildStagingCleanup { path, error, prior } => {
                write!(
                    formatter,
                    "could not dispose package-review build session `{}`: {error}",
                    path.display()
                )?;
                if let Some(prior) = prior {
                    write!(formatter, "; review had already failed: {prior}")?;
                }
                Ok(())
            }
            Self::SourceCustody {
                compiling_package,
                source_package,
                phase,
                error,
            } => write!(
                formatter,
                "source custody verification failed {phase:?} for package `{}` while compiling `{}`: {error}",
                source_package.name().as_str(),
                compiling_package.name().as_str()
            ),
            Self::SourceSelectionCustody {
                compiling_package,
                source_package,
                phase,
                error,
            } => write!(
                formatter,
                "source selection verification failed {phase:?} for package `{}` while compiling `{}`: {error}",
                source_package.name().as_str(),
                compiling_package.name().as_str()
            ),
            Self::SemanticBindingConsumerAbsent { consumer, role } => write!(
                formatter,
                "semantic-binding review input for role {role:?} names consumer `{}` outside the resolved package closure",
                consumer.name().as_str()
            ),
            Self::DuplicateConsumerSemanticBindingRole { consumer, role } => write!(
                formatter,
                "semantic-binding review inputs contain duplicate role {role:?} for consumer `{}`",
                consumer.name().as_str()
            ),
            Self::AmbiguousCandidateSemanticBinding {
                consumer,
                role,
                candidate_count,
            } => write!(
                formatter,
                "candidate review found {candidate_count} package-owned declarations for semantic role {role:?} in consumer `{}`; expected at most one",
                consumer.name().as_str()
            ),
            Self::InvalidCandidateSemanticBinding { consumer, role } => write!(
                formatter,
                "candidate review could not construct exact semantic role {role:?} for consumer `{}`",
                consumer.name().as_str()
            ),
            Self::CompilationInputs { package, errors } => {
                write!(
                    formatter,
                    "compiler input validation failed for package `{}` with {} error(s)",
                    package.name().as_str(),
                    errors.len()
                )?;
                for error in errors {
                    write!(formatter, "; {error}")?;
                }
                Ok(())
            }
            Self::Compilation {
                package,
                diagnostics,
            } => diagnostic_output::render(formatter, "checked compilation", package, diagnostics),
            Self::Projection {
                package,
                diagnostics,
            } => diagnostic_output::render(formatter, "review projection", package, diagnostics),
            Self::Encoding { package, error } => write!(
                formatter,
                "review encoding failed for package `{}`: {error}",
                package.name().as_str()
            ),
            Self::SourceConsumptionMissing { package } => write!(
                formatter,
                "package-aware compilation for `{}` emitted no source-consumption commitment",
                package.name().as_str()
            ),
            Self::SourceConsumptionDrift {
                package,
                diagnostics,
            } => diagnostic_output::render(
                formatter,
                "compiler-consumed source verification",
                package,
                diagnostics,
            ),
            Self::IdentityMismatch { package } => write!(
                formatter,
                "compiler review identity did not match package `{}`",
                package.name().as_str()
            ),
            Self::InvalidProductionRootRole { package, role } => write!(
                formatter,
                "reviewed native production requires application root `{}`; found {role:?}",
                package.name().as_str()
            ),
            Self::RetainedPolicyCanonicalBudget {
                package,
                maximum_bytes,
            } => write!(
                formatter,
                "normalized package policies exceeded the {maximum_bytes}-byte aggregate canonical encoding ceiling while compiling package `{}`",
                package.name().as_str()
            ),
            Self::ComponentDescriptionNotRetained { package, .. } => write!(
                formatter,
                "an independent provider selection in package `{}` names a dependency whose review retains no component description",
                package.name().as_str()
            ),
        }
    }
}

impl std::error::Error for CompileResolvedPackageReviewsError {}
