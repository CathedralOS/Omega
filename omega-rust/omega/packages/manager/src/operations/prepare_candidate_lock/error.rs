use crate::declarations::PackageKey;
use crate::lock::{HistoricalPackagePolicyError, PackageLockError};
use crate::resolution::graph::CanonicalSourceClosureSubjectError;
use crate::resolution::source::PackageSourceSelectionEvidenceError;
use crate::review::{PackagePolicyChangeError, PackagePolicyDecisionError};
use omega_package_source::SourceResolveError;
use std::fmt;

#[derive(Debug)]
pub enum PrepareCandidateLockError {
    Comparison(PackagePolicyChangeError),
    Decisions(PackagePolicyDecisionError),
    ResolutionMismatch,
    RejectedDecision,
    OpenContractEntailment {
        package: PackageKey,
    },
    ObligationAssociation {
        package: PackageKey,
    },
    CompilerInput {
        package: PackageKey,
        errors: Vec<omega_package_compilation::PackageCompilationInputError>,
    },
    GeneratedSourceAssociation {
        package: PackageKey,
    },
    SourceSubject(CanonicalSourceClosureSubjectError),
    SourceSnapshot {
        package: PackageKey,
        error: SourceResolveError,
    },
    SourceSelection {
        package: PackageKey,
        error: PackageSourceSelectionEvidenceError,
    },
    History(HistoricalPackagePolicyError),
    Lock(PackageLockError),
    AllocationFailed,
}
impl fmt::Display for PrepareCandidateLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Comparison(error) => {
                write!(formatter, "candidate lock comparison failed: {error}")
            }
            Self::Decisions(error) => {
                write!(formatter, "candidate lock decisions are invalid: {error}")
            }
            Self::ResolutionMismatch => formatter.write_str(
                "candidate lock requires the exact complete current decision resolution",
            ),
            Self::RejectedDecision => {
                formatter.write_str("project policy rejects a required candidate change")
            }
            Self::OpenContractEntailment { package } => write!(
                formatter,
                "package {package:?} has an open contract entailment that project policy cannot discharge"
            ),
            Self::ObligationAssociation { package } => write!(
                formatter,
                "package {package:?} has inconsistent checked obligation associations"
            ),
            Self::CompilerInput { package, errors } => write!(
                formatter,
                "cannot revalidate compiler dependency inputs for {package:?}: {errors:?}"
            ),
            Self::GeneratedSourceAssociation { package } => write!(
                formatter,
                "package {package:?} has inconsistent checked generated-source associations"
            ),
            Self::SourceSubject(error) => write!(
                formatter,
                "candidate lock source subject is invalid: {error}"
            ),
            Self::SourceSnapshot { package, error } => write!(
                formatter,
                "candidate source snapshot changed for {package:?}: {error}"
            ),
            Self::SourceSelection { package, error } => write!(
                formatter,
                "candidate source selection changed for {package:?}: {error}"
            ),
            Self::History(error) => {
                write!(formatter, "candidate decision history is invalid: {error}")
            }
            Self::Lock(error) => write!(formatter, "candidate lock target is invalid: {error}"),
            Self::AllocationFailed => {
                formatter.write_str("candidate lock collection allocation failed")
            }
        }
    }
}
impl std::error::Error for PrepareCandidateLockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Comparison(error) => Some(error),
            Self::Decisions(error) => Some(error),
            Self::SourceSubject(error) => Some(error),
            Self::SourceSnapshot { error, .. } => Some(error),
            Self::SourceSelection { error, .. } => Some(error),
            Self::History(error) => Some(error),
            Self::Lock(error) => Some(error),
            _ => None,
        }
    }
}
