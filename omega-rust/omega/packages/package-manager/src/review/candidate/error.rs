/*
    We define `CompileResolvedPackageReviewsError` for failures encountered
    while staging build sessions, verifying source custody, resolving semantic
    bindings, compiling occurrences, and projecting normalized policy rows.

    Pending restricted build consent (`CompileFailure::BuildConsentRequired`) is
    preserved at the package boundary via `from_compilation` rather than parsed
    from diagnostic strings, and `render_package_diagnostics` bounds displayed
    package names and compiler diagnostics.
*/

use super::PackageSourceVerificationPhase;
use crate::declarations::PackageKey;
use crate::lock::PackageCheckedContext;
use crate::resolution::graph::DependencyRequestPath;
use crate::resolution::source::PackageSourceSelectionEvidenceError;
use crate::review::restricted_build_grants::UngrantedRestrictedBuildRequest;
use compilation_products::CompileFailure;
use diagnostics::Diagnostic;
use diagnostics::write_bounded_diagnostics;
use filesystem_authority::FilesystemSponsorError;
use package_compilation::{
    AcceptedSemanticBindingRole, BuildDeclarationKind, PackageCompilationInputError,
};
use package_evidence::encoding::PackageReviewEncodingError;
use package_source::SourceResolveError;
use std::fmt;
use std::io;
use std::path::PathBuf;

const MAXIMUM_PACKAGE_NAME_BYTES: usize = 256;

#[derive(Debug)]
pub enum CompileResolvedPackageReviewCause {
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
        prior: Option<String>,
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
        failure: CompileFailure,
    },
    BuildConsentRequired {
        package: PackageKey,
        requests: Vec<UngrantedRestrictedBuildRequest>,
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
    ComponentDescriptionNotRetained {
        package: PackageKey,
        component: semantic_vocabulary::PackageKeyIdentity,
        purpose: crate::declarations::DependencyPurpose,
    },
}

/// One review attempt retains its original typed cause and actual work.
/// Cost history is not a policy row, cache result or execution permission.
#[derive(Debug)]
pub struct CompileResolvedPackageReviewsError {
    pub cause: CompileResolvedPackageReviewCause,
    accounting: compilation_products::SourceCompletionAccounting,
}

impl CompileResolvedPackageReviewsError {
    pub fn new(cause: CompileResolvedPackageReviewCause) -> Self {
        let accounting = match &cause {
            CompileResolvedPackageReviewCause::Compilation { failure, .. } => failure
                .source_completion_accounting()
                .cloned()
                .unwrap_or_default(),
            _ => Default::default(),
        };
        Self { cause, accounting }
    }

    pub fn source_completion_accounting(
        &self,
    ) -> Option<&compilation_products::SourceCompletionAccounting> {
        (!self.accounting.receipts().is_empty()).then_some(&self.accounting)
    }

    pub fn with_source_completion_accounting(
        mut self,
        accounting: compilation_products::SourceCompletionAccounting,
    ) -> Self {
        self.accounting.append(accounting);
        self
    }

    pub(super) fn compilation_inputs(
        package: &PackageKey,
        errors: Vec<PackageCompilationInputError>,
    ) -> Self {
        Self::new(CompileResolvedPackageReviewCause::CompilationInputs {
            package: package.clone(),
            errors,
        })
    }

    pub(crate) fn from_compilation(
        package: PackageKey,
        context: PackageCheckedContext,
        request_path: Option<DependencyRequestPath>,
        failure: CompileFailure,
    ) -> Self {
        let accounting = failure
            .source_completion_accounting()
            .cloned()
            .unwrap_or_default();
        let cause = match failure {
            failure
            @ (CompileFailure::Diagnostics(_) | CompileFailure::SourceCompletion { .. }) => {
                CompileResolvedPackageReviewCause::Compilation { package, failure }
            }
            CompileFailure::BuildConsentRequired { requests, .. } => {
                let projected = requests
                    .iter()
                    .map(|request| {
                        UngrantedRestrictedBuildRequest::project(
                            package.identity(),
                            context,
                            request_path.clone(),
                            request,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>();
                match projected {
                    Ok(requests) => CompileResolvedPackageReviewCause::BuildConsentRequired {
                        package,
                        requests,
                    },
                    Err(error) => CompileResolvedPackageReviewCause::Encoding { package, error },
                }
            }
        };
        Self::new(cause).with_source_completion_accounting(accounting)
    }
}

fn render_package_diagnostics(
    formatter: &mut fmt::Formatter<'_>,
    phase: &str,
    package: &PackageKey,
    diagnostics: &[Diagnostic],
) -> fmt::Result {
    let name = package.name().as_str();
    let shown_name = name.len().min(MAXIMUM_PACKAGE_NAME_BYTES);
    write!(
        formatter,
        "{phase} failed for package `{0}",
        &name[..shown_name]
    )?;
    if shown_name != name.len() {
        write!(
            formatter,
            "...[{} name bytes omitted]",
            name.len() - shown_name
        )?;
    }
    write!(formatter, "` with {} diagnostic(s)", diagnostics.len())?;
    write_bounded_diagnostics(formatter, diagnostics)
}

impl fmt::Display for CompileResolvedPackageReviewsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            CompileResolvedPackageReviewCause::BuildConsentRequired {
                package, requests, ..
            } => {
                write!(
                    formatter,
                    "package `{}` Build review is pending explicit execution consent; generated-source and final product findings are unavailable",
                    package.name().as_str(),
                )?;
                for request in requests {
                    write!(formatter, "\n{request}")?;
                }
                Ok(())
            }
            CompileResolvedPackageReviewCause::SemanticBindingContextMismatch {
                consumer,
                role,
            } => write!(
                formatter,
                "semantic binding {role:?} for package `{}` names an unrequested checked occurrence",
                consumer.name().as_str()
            ),
            CompileResolvedPackageReviewCause::UnsupportedBuildActivation { package, reason } => {
                write!(
                    formatter,
                    "cannot schedule build activation for package `{}`: {reason}",
                    package.name().as_str()
                )
            }
            CompileResolvedPackageReviewCause::BuildStagingCreate { path, error } => write!(
                formatter,
                "could not create a fresh package-review build session at `{}`: {error}",
                path.display()
            ),
            CompileResolvedPackageReviewCause::BuildStagingSponsor { path, error } => write!(
                formatter,
                "failed to sponsor package-review staging root `{}`: {error}",
                path.display()
            ),
            CompileResolvedPackageReviewCause::BuildStagingCleanup { path, error, prior } => {
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
            CompileResolvedPackageReviewCause::SourceCustody {
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
            CompileResolvedPackageReviewCause::SourceSelectionCustody {
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
            CompileResolvedPackageReviewCause::SemanticBindingConsumerAbsent { consumer, role } => {
                write!(
                    formatter,
                    "semantic-binding review input for role {role:?} names consumer `{}` outside the resolved package closure",
                    consumer.name().as_str()
                )
            }
            CompileResolvedPackageReviewCause::DuplicateConsumerSemanticBindingRole {
                consumer,
                role,
            } => write!(
                formatter,
                "semantic-binding review inputs contain duplicate role {role:?} for consumer `{}`",
                consumer.name().as_str()
            ),
            CompileResolvedPackageReviewCause::AmbiguousCandidateSemanticBinding {
                consumer,
                role,
                candidate_count,
            } => write!(
                formatter,
                "candidate review found {candidate_count} package-owned declarations for semantic role {role:?} in consumer `{}`; expected at most one",
                consumer.name().as_str()
            ),
            CompileResolvedPackageReviewCause::InvalidCandidateSemanticBinding {
                consumer,
                role,
            } => write!(
                formatter,
                "candidate review could not construct exact semantic role {role:?} for consumer `{}`",
                consumer.name().as_str()
            ),
            CompileResolvedPackageReviewCause::CompilationInputs { package, errors } => {
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
            CompileResolvedPackageReviewCause::Compilation { package, failure } => match failure {
                CompileFailure::Diagnostics(diagnostics)
                | CompileFailure::SourceCompletion { diagnostics, .. } => {
                    render_package_diagnostics(
                        formatter,
                        "checked compilation",
                        package,
                        diagnostics,
                    )
                }
                CompileFailure::BuildConsentRequired { requests, .. } => write!(
                    formatter,
                    "restricted Build execution requires explicit consent; pending requests: {requests:?}"
                ),
            },
            CompileResolvedPackageReviewCause::Projection {
                package,
                diagnostics,
            } => render_package_diagnostics(formatter, "review projection", package, diagnostics),
            CompileResolvedPackageReviewCause::Encoding { package, error, .. } => write!(
                formatter,
                "review encoding failed for package `{}`: {error}",
                package.name().as_str()
            ),
            CompileResolvedPackageReviewCause::SourceConsumptionMissing { package } => write!(
                formatter,
                "package-aware compilation for `{}` emitted no source-consumption commitment",
                package.name().as_str()
            ),
            CompileResolvedPackageReviewCause::SourceConsumptionDrift {
                package,
                diagnostics,
            } => render_package_diagnostics(
                formatter,
                "compiler-consumed source verification",
                package,
                diagnostics,
            ),
            CompileResolvedPackageReviewCause::IdentityMismatch { package } => write!(
                formatter,
                "compiler review identity did not match package `{}`",
                package.name().as_str()
            ),
            CompileResolvedPackageReviewCause::InvalidProductionRootRole { package, role } => {
                write!(
                    formatter,
                    "reviewed native production requires application root `{}`; found {role:?}",
                    package.name().as_str()
                )
            }
            CompileResolvedPackageReviewCause::RetainedPolicyCanonicalBudget {
                package,
                maximum_bytes,
            } => write!(
                formatter,
                "normalized package policies exceeded the {maximum_bytes}-byte aggregate canonical encoding ceiling while compiling package `{}`",
                package.name().as_str()
            ),
            CompileResolvedPackageReviewCause::ComponentDescriptionNotRetained {
                package, ..
            } => write!(
                formatter,
                "an independent provider selection in package `{}` names a dependency whose review retains no component description",
                package.name().as_str()
            ),
        }
    }
}

impl std::error::Error for CompileResolvedPackageReviewsError {}
