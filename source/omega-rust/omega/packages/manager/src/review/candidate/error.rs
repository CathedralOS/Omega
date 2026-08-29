use super::PackageSourceVerificationPhase;
use crate::discovery::PackageSourceSelectionEvidenceError;
use crate::identity::PackageKey;
use omega_package_compilation::PackageCompilationInputError;
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
    RetainedObligationLedgerBudget {
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
            Self::RetainedObligationLedgerBudget {
                package,
                maximum_bytes,
            } => write!(
                formatter,
                "retained ordinary obligation ledgers exceeded the {maximum_bytes}-byte review-session ceiling while compiling package `{}`",
                package.name().as_str()
            ),
        }
    }
}

impl std::error::Error for CompileResolvedPackageReviewsError {}
