use super::PackageSourceVerificationPhase;
use crate::declarations::PackageKey;
use crate::resolution::source::PackageSourceSelectionEvidenceError;
use omega_package_compilation::{
    AcceptedSemanticBindingRole, BuildDeclarationKind, PackageCompilationInputError,
};
use omega_package_evidence::encoding::PackageReviewEncodingError;
use omega_package_source::SourceResolveError;
use psi_checked_interpreter::FilesystemSponsorError;
use psi_diagnostics::Diagnostic;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CompileResolvedPackageReviewsError {
    BuildStagingCreate {
        path: PathBuf,
        error: io::Error,
    },
    BuildStagingSponsor {
        path: PathBuf,
        error: FilesystemSponsorError,
    },
    BuildEvaluationAccountingMismatch {
        reported: Option<u64>,
        sponsored: u64,
    },
    BuildLogAccountingMismatch {
        reported: Option<u64>,
        sponsored: u64,
    },
    BuildFilesystemAttemptAccountingMismatch {
        reported: Option<u64>,
        sponsored: u64,
    },
    BuildLiveFilesystemHandleAccountingMismatch {
        reported_peak: Option<u64>,
        sponsored_peak: u64,
        sponsored_live: u64,
    },
    BuildLiveCellAccountingMismatch {
        reported_invocation_peak: Option<u64>,
        reported_session_peak: Option<u64>,
        sponsored_peak: u64,
        sponsored_live: u64,
    },
    BuildLiveTextByteAccountingMismatch {
        reported_invocation_peak: Option<u64>,
        reported_session_peak: Option<u64>,
        sponsored_peak: u64,
        sponsored_live: u64,
    },
    BuildResultCustodyAccountingMismatch {
        reported_cells: Option<u64>,
        sponsored_cells: u64,
        reported_text_bytes: Option<u64>,
        sponsored_text_bytes: u64,
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
    RetainedObligationLedgerBudget {
        package: PackageKey,
        maximum_bytes: usize,
    },
    RetainedPolicyCanonicalBudget {
        package: PackageKey,
        maximum_bytes: usize,
    },
}

impl fmt::Display for CompileResolvedPackageReviewsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::BuildEvaluationAccountingMismatch {
                reported,
                sponsored,
            } => write!(
                formatter,
                "package-review build usage did not reconcile with its shared evaluator sponsor (reported {reported:?}, sponsored {sponsored})"
            ),
            Self::BuildLogAccountingMismatch {
                reported,
                sponsored,
            } => write!(
                formatter,
                "package-review BuildLog usage did not reconcile with its shared evaluator sponsor (reported {reported:?}, sponsored {sponsored})"
            ),
            Self::BuildFilesystemAttemptAccountingMismatch {
                reported,
                sponsored,
            } => write!(
                formatter,
                "package-review filesystem-attempt usage did not reconcile with its shared evaluator sponsor (reported {reported:?}, sponsored {sponsored})"
            ),
            Self::BuildLiveFilesystemHandleAccountingMismatch {
                reported_peak,
                sponsored_peak,
                sponsored_live,
            } => write!(
                formatter,
                "package-review live-filesystem-handle usage did not reconcile with its shared evaluator sponsor (reported peak {reported_peak:?}, sponsored peak {sponsored_peak}, still live {sponsored_live})"
            ),
            Self::BuildLiveCellAccountingMismatch {
                reported_invocation_peak,
                reported_session_peak,
                sponsored_peak,
                sponsored_live,
            } => write!(
                formatter,
                "package-review live-cell usage did not reconcile with its shared evaluator sponsor (reported invocation peak {reported_invocation_peak:?}, reported session peak {reported_session_peak:?}, sponsored peak {sponsored_peak}, still live {sponsored_live})"
            ),
            Self::BuildLiveTextByteAccountingMismatch {
                reported_invocation_peak,
                reported_session_peak,
                sponsored_peak,
                sponsored_live,
            } => write!(
                formatter,
                "package-review live-Text-byte usage did not reconcile with its shared evaluator sponsor (reported invocation peak {reported_invocation_peak:?}, reported session peak {reported_session_peak:?}, sponsored peak {sponsored_peak}, still live {sponsored_live})"
            ),
            Self::BuildResultCustodyAccountingMismatch {
                reported_cells,
                sponsored_cells,
                reported_text_bytes,
                sponsored_text_bytes,
            } => write!(
                formatter,
                "package-review result custody did not reconcile with its shared evaluator sponsor (reported cells {reported_cells:?}, sponsored cells {sponsored_cells}, reported Text bytes {reported_text_bytes:?}, sponsored Text bytes {sponsored_text_bytes})"
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
            Self::CompilationInputs { package, errors } => write!(
                formatter,
                "compiler input validation failed for package `{}` with {} error(s)",
                package.name().as_str(),
                errors.len()
            ),
            Self::Compilation {
                package,
                diagnostics,
            } => write!(
                formatter,
                "checked compilation failed for package `{}` with {} diagnostic(s)",
                package.name().as_str(),
                diagnostics.len()
            ),
            Self::Projection {
                package,
                diagnostics,
            } => write!(
                formatter,
                "review projection failed for package `{}` with {} diagnostic(s)",
                package.name().as_str(),
                diagnostics.len()
            ),
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
            } => write!(
                formatter,
                "compiler-consumed source verification failed for package `{}` with {} diagnostic(s)",
                package.name().as_str(),
                diagnostics.len()
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
            Self::RetainedObligationLedgerBudget {
                package,
                maximum_bytes,
            } => write!(
                formatter,
                "retained ordinary obligation ledgers exceeded the {maximum_bytes}-byte review-session ceiling while compiling package `{}`",
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
        }
    }
}

impl std::error::Error for CompileResolvedPackageReviewsError {}
