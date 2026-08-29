use super::custody::{
    dependency_first_package_order, package_build_root, verify_transitive_source_custody,
};
use super::ledger::{
    MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES, reserve_retained_obligation_ledger_bytes,
    retained_obligation_ledger_bytes,
};
use super::session::ReviewBuildSession;
use super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReview,
    CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
};
use crate::graph::ResolvedPackageSourceClosure;
use crate::review::compilation::inputs::reachable_package_keys;
use crate::review::package_compilation_inputs_for;
use crate::review::records::ReviewOnlyCanonicalRow;
use omega_compiler::compile_to_checked_with_packages_in_sponsored_build_dir;
use omega_package_compilation::PackageCompilationInputError;
use omega_package_review::obligation_ledger::{
    ordinary_package_obligation_ledger_from_compiler_rows,
    validate_ordinary_package_obligation_ledger,
};
use omega_package_review::project_checked_package_review;
use psi_checked_interpreter::FilesystemSponsor;
use psi_diagnostics::Diagnostic;
use std::path::Path;

/// Compile every package in an exact resolver-owned closure and project its
/// review material locally.
///
/// Each package is temporarily re-rooted over only its transitive dependencies
/// and receives a source-specific writable directory within a fresh disposable
/// review session. Downloaded source remains immutable and cannot supply its
/// own review rows. No review set is returned until the session is removed.
pub fn compile_resolved_package_reviews(
    closure: &ResolvedPackageSourceClosure,
    target: &str,
    build_root: &Path,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    let build_session = ReviewBuildSession::create(build_root)?;
    let result = compile_resolved_package_reviews_in_session(
        closure,
        target,
        build_session.root(),
        build_session.sponsor(),
    );
    build_session.dispose(result)
}

fn compile_resolved_package_reviews_in_session(
    closure: &ResolvedPackageSourceClosure,
    target: &str,
    build_session_root: &Path,
    filesystem_sponsor: &FilesystemSponsor,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    let mut reviews = Vec::<CompilerIssuedPackageReview>::with_capacity(closure.custodies().len());
    let mut retained_obligation_ledger_total = 0usize;
    for key in dependency_first_package_order(closure) {
        verify_transitive_source_custody(
            closure,
            &key,
            PackageSourceVerificationPhase::BeforeCompilation,
        )?;
        let custody = closure
            .custody(&key)
            .expect("validated source closure retains custody for every graph package");
        let inputs = package_compilation_inputs_for(closure, &key).map_err(|errors| {
            CompileResolvedPackageReviewsError::CompilationInputs {
                package: key.clone(),
                errors,
            }
        })?;
        let dependency_bundles = reachable_package_keys(closure, &key)
            .into_iter()
            .filter(|dependency| dependency != &key)
            .map(|dependency| {
                let review = reviews
                    .iter()
                    .find(|review| review.key() == &dependency)
                    .ok_or(PackageCompilationInputError::MissingGeneratedSourceBundle {
                        package: dependency.identity(),
                    })?;
                let custody = closure.custody(&dependency).ok_or(
                    PackageCompilationInputError::ForeignGeneratedSourceBundle {
                        package: dependency.identity(),
                    },
                )?;
                let bundle = review.generated_source_bundle();
                if review.resolution() != custody.resolution()
                    || bundle.package() != dependency.identity()
                    || bundle.source_consumption_commitment()
                        != review.source_consumption_commitment()
                {
                    return Err(
                        PackageCompilationInputError::GeneratedSourceBundleCustodyMismatch {
                            package: dependency.identity(),
                        },
                    );
                }
                Ok(bundle.clone())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(
                |error| CompileResolvedPackageReviewsError::CompilationInputs {
                    package: key.clone(),
                    errors: vec![error],
                },
            )?;
        let inputs = inputs
            .with_complete_dependency_generated_sources(dependency_bundles)
            .map_err(
                |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                    package: key.clone(),
                    errors,
                },
            )?;
        let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
            &custody.snapshot_root().join("main.omg"),
            &package_build_root(build_session_root, &key, custody.resolution()),
            Some(target),
            inputs,
            filesystem_sponsor.clone(),
        )
        .map_err(
            |diagnostics| CompileResolvedPackageReviewsError::Compilation {
                package: key.clone(),
                diagnostics,
            },
        )?;
        verify_transitive_source_custody(
            closure,
            &key,
            PackageSourceVerificationPhase::AfterCompilation,
        )?;
        checked
            .verify_current_source_consumption()
            .map_err(
                |diagnostics| CompileResolvedPackageReviewsError::SourceConsumptionDrift {
                    package: key.clone(),
                    diagnostics,
                },
            )?;
        let source_consumption_commitment =
            checked.source_consumption_commitment().ok_or_else(|| {
                CompileResolvedPackageReviewsError::SourceConsumptionMissing {
                    package: key.clone(),
                }
            })?;
        let build_observation_summary = checked.build_observation_summary().cloned();
        let generated_source_bundle =
            checked.package_generated_source_bundle().map_err(|error| {
                CompileResolvedPackageReviewsError::Projection {
                    package: key.clone(),
                    diagnostics: vec![Diagnostic::error(error)],
                }
            })?;
        if generated_source_bundle.package() != key.identity()
            || generated_source_bundle.source_consumption_commitment()
                != source_consumption_commitment
        {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch { package: key });
        }
        let projection = project_checked_package_review(&checked).map_err(|diagnostics| {
            CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics,
            }
        })?;
        if projection.package() != key.identity() {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch { package: key });
        }
        let canonical_review_bytes = projection.canonical_review_bytes().map_err(|error| {
            CompileResolvedPackageReviewsError::Encoding {
                package: key.clone(),
                error,
            }
        })?;
        let canonical_rows = projection.canonical_rows().map_err(|error| {
            CompileResolvedPackageReviewsError::Encoding {
                package: key.clone(),
                error,
            }
        })?;
        let dependency_closure = checked.dependency_closure().cloned().ok_or_else(|| {
            CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics: vec![Diagnostic::error(
                    "package-aware review compilation emitted no dependency closure",
                )],
            }
        })?;
        let obligation_ledger = ordinary_package_obligation_ledger_from_compiler_rows(
            dependency_closure,
            &canonical_rows,
        )
        .map_err(|error| CompileResolvedPackageReviewsError::Projection {
            package: key.clone(),
            diagnostics: vec![Diagnostic::error(format!(
                "compiler-issued ordinary package obligation ledger is structurally invalid: {error}"
            ))],
        })?;
        validate_ordinary_package_obligation_ledger(&obligation_ledger, &checked).map_err(
            |diagnostics| CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics,
            },
        )?;
        let obligation_ledger_bytes = retained_obligation_ledger_bytes(&obligation_ledger)
            .ok_or_else(
                || CompileResolvedPackageReviewsError::RetainedObligationLedgerBudget {
                    package: key.clone(),
                    maximum_bytes: MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES,
                },
            )?;
        retained_obligation_ledger_total = reserve_retained_obligation_ledger_bytes(
            retained_obligation_ledger_total,
            obligation_ledger_bytes,
        )
        .ok_or_else(|| {
            CompileResolvedPackageReviewsError::RetainedObligationLedgerBudget {
                package: key.clone(),
                maximum_bytes: MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES,
            }
        })?;
        let comparison_rows = canonical_rows
            .iter()
            .map(ReviewOnlyCanonicalRow::from_compiler_issued)
            .collect();
        reviews.push(CompilerIssuedPackageReview {
            key: key.clone(),
            resolution: custody.resolution().clone(),
            source_consumption_commitment,
            build_observation_summary,
            generated_source_bundle: generated_source_bundle.clone(),
            projection,
            canonical_review_bytes,
            canonical_rows,
            obligation_ledger,
            comparison_rows,
        });
    }
    Ok(CompilerIssuedPackageReviewSet { reviews })
}
