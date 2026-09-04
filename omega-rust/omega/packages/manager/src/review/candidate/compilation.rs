//! Candidate review compilation and disposable-session coordination.
//!
//! Public routes establish semantic-binding policy and session lifetime. The
//! dependency-first loop compiles and projects each package; the adjacent
//! `session_accounting` leaf independently reconciles aggregate sponsor use.

mod session_accounting;

use session_accounting::verify_build_session_accounting;

use super::custody::{
    dependency_first_package_order, package_build_root, verify_transitive_source_custody,
};
use super::ledger::{
    MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES, reserve_retained_obligation_ledger_bytes,
    retained_obligation_ledger_bytes,
};
use super::rows::ReviewOnlyCanonicalRow;
use super::semantic_bindings::{
    ConsumerScopedSemanticBindingReviewInput, candidate_semantic_binding_inputs,
    candidate_service_bindings, semantic_bindings_by_consumer,
};
use super::session::ReviewBuildSession;
use super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReview,
    CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
    ReviewedPackageProductionCandidate,
};
use crate::declarations::PackageKey;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::resolution::{package_compilation_inputs_for, reachable_package_keys};
use omega_compiler::compile_to_checked_with_packages_in_sponsored_build_session;
use omega_package_compilation::{AcceptedSemanticBinding, PackageCompilationInputError};
use omega_package_evidence::ledger::{
    ordinary_package_obligation_ledger_from_compiler_rows,
    reconstruct_ordinary_package_obligation_results,
};
use omega_package_evidence::project_checked_package_review;
use psi_checked_interpreter::{BuildEvaluationSponsor, FilesystemSponsor};
use psi_diagnostics::Diagnostic;
use std::collections::BTreeMap;
use std::path::Path;

/// Compile every package in an exact resolver-owned closure and project its
/// review material locally.
///
/// Each package is temporarily re-rooted over only its transitive dependencies
/// and receives a source-specific writable directory within a fresh disposable
/// review session. Downloaded source remains immutable and cannot supply its
/// own review rows. No review set is returned until the session is removed.
pub fn compile_resolved_package_reviews(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    compile_resolved_package_reviews_with_semantic_bindings(target_closure, build_root, &[])
}

/// Compile one install/update candidate through semantic-binding discovery.
///
/// The preliminary pass can only propose exact package-owned surfaces. Any
/// proposal is recompiled as consumer-scoped policy input so the compiler must
/// consume it and the final review exposes every resulting policy blocker.
pub fn compile_resolved_package_candidate_reviews(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    let preliminary = compile_resolved_package_reviews(target_closure, build_root)?;
    let semantic_binding_inputs = candidate_semantic_binding_inputs(&preliminary)?;
    if semantic_binding_inputs.is_empty() {
        return Ok(preliminary);
    }
    compile_resolved_package_reviews_with_semantic_bindings(
        target_closure,
        build_root,
        &semantic_binding_inputs,
    )
}

/// Compile one install/update candidate and retain its exact final checked
/// application root for later accepted production.
pub fn compile_resolved_package_candidate_for_production(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
) -> Result<ReviewedPackageProductionCandidate, CompileResolvedPackageReviewsError> {
    let preliminary = compile_resolved_package_candidate_for_production_with_semantic_bindings(
        target_closure,
        build_root,
        &[],
    )?;
    let semantic_binding_inputs = candidate_semantic_binding_inputs(preliminary.reviews())?;
    if semantic_binding_inputs.is_empty() {
        return Ok(preliminary);
    }
    compile_resolved_package_candidate_for_production_with_semantic_bindings(
        target_closure,
        build_root,
        &semantic_binding_inputs,
    )
}

/// Compile one candidate with explicit consumer policy and retain the checked
/// root from that same final review pass.
pub fn compile_resolved_package_candidate_for_production_with_semantic_bindings(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    semantic_binding_inputs: &[ConsumerScopedSemanticBindingReviewInput],
) -> Result<ReviewedPackageProductionCandidate, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let root = closure.graph().root().clone();
    if closure.root_role() != omega_package_compilation::BuildDeclarationKind::Application {
        return Err(
            CompileResolvedPackageReviewsError::InvalidProductionRootRole {
                package: root,
                role: closure.root_role(),
            },
        );
    }
    let semantic_bindings_by_consumer =
        semantic_bindings_by_consumer(closure, semantic_binding_inputs)?;
    let build_session = ReviewBuildSession::create(build_root)?;
    let result = compile_resolved_package_reviews_in_session(
        target_closure,
        build_session.root(),
        build_session.filesystem_sponsor(),
        build_session.evaluation_sponsor(),
        &semantic_bindings_by_consumer,
        true,
    );
    let compiled = build_session.dispose(result)?;
    let root_path = closure
        .source_root(&root)
        .expect("validated source closure retains its root custody")
        .join("main.omg");
    let checked_root = compiled.checked_root.ok_or_else(|| {
        CompileResolvedPackageReviewsError::IdentityMismatch {
            package: root.clone(),
        }
    })?;
    let checked_subject = checked_root.package_compilation_subject();
    if checked_subject.map(|subject| subject.root()) != Some(root.identity())
        || checked_subject.map(|subject| subject.root_role()) != Some(closure.root_role())
        || checked_root.selected_target_profile() != Some(target_closure.target_profile())
    {
        return Err(CompileResolvedPackageReviewsError::IdentityMismatch { package: root });
    }
    Ok(ReviewedPackageProductionCandidate {
        reviews: compiled.reviews,
        root,
        root_path,
        root_role: closure.root_role(),
        target_profile: target_closure.target_profile(),
        checked_root,
    })
}

/// Compile candidate reviews with explicit consumer-policy semantic bindings.
///
/// Inputs are scoped to one exact consumer. Each package receives only its own
/// rows after it has been re-rooted over its transitive dependency closure.
/// Provider membership and binding contents remain compiler-input invariants;
/// these rows are not proof/audit receipts and do not admit a package.
pub fn compile_resolved_package_reviews_with_semantic_bindings(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    semantic_binding_inputs: &[ConsumerScopedSemanticBindingReviewInput],
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let semantic_bindings_by_consumer =
        semantic_bindings_by_consumer(closure, semantic_binding_inputs)?;
    let build_session = ReviewBuildSession::create(build_root)?;
    let result = compile_resolved_package_reviews_in_session(
        target_closure,
        build_session.root(),
        build_session.filesystem_sponsor(),
        build_session.evaluation_sponsor(),
        &semantic_bindings_by_consumer,
        false,
    );
    build_session
        .dispose(result)
        .map(|compiled| compiled.reviews)
}

struct CompiledPackageReviews {
    reviews: CompilerIssuedPackageReviewSet,
    checked_root: Option<omega_compiler::CheckedCompilation>,
}

fn compile_resolved_package_reviews_in_session(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_session_root: &Path,
    filesystem_sponsor: &FilesystemSponsor,
    evaluation_sponsor: &BuildEvaluationSponsor,
    semantic_bindings_by_consumer: &BTreeMap<PackageKey, Vec<AcceptedSemanticBinding>>,
    retain_checked_root: bool,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let target = target_closure.target_profile().target_name();
    let mut reviews = Vec::<CompilerIssuedPackageReview>::with_capacity(closure.custodies().len());
    let mut checked_root = None;
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
        let inputs = inputs
            .with_accepted_semantic_bindings(
                semantic_bindings_by_consumer
                    .get(&key)
                    .cloned()
                    .unwrap_or_default(),
            )
            .map_err(
                |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                    package: key.clone(),
                    errors,
                },
            )?;
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
        let checked = compile_to_checked_with_packages_in_sponsored_build_session(
            &custody.snapshot_root().join("main.omg"),
            &package_build_root(build_session_root, &key, custody.resolution()),
            Some(target),
            inputs,
            filesystem_sponsor.clone(),
            evaluation_sponsor.clone(),
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
        let selected_build_machine_identity = checked
            .selected_build_machine_identity()
            .ok_or_else(|| CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics: vec![Diagnostic::error(
                    "package review requires one exact selected build-machine identity",
                )],
            })?
            .to_owned();
        let build_observation_summary = checked.build_observation_summary().cloned();
        let build_evaluation_usage = checked.build_evaluation_usage();
        let semantic_bindings = checked.resolved_semantic_bindings().cloned().collect();
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
        let semantic_binding_candidates = candidate_service_bindings(&checked, &projection, &key)?;
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
        let obligations = ordinary_package_obligation_ledger_from_compiler_rows(
            dependency_closure,
            &canonical_rows,
        )
        .map_err(|error| CompileResolvedPackageReviewsError::Projection {
            package: key.clone(),
            diagnostics: vec![Diagnostic::error(format!(
                "compiler-issued ordinary package obligation ledger is structurally invalid: {error}"
            ))],
        })?;
        // Reconstruct results once, including certificate association and proof
        // rechecking. Fresh outputs need no second reconstruction against this
        // unchanged checked compilation; external evidence validators remain
        // responsible for comparing supplied results with current semantics.
        let obligation_results = reconstruct_ordinary_package_obligation_results(&checked)
            .map_err(
                |diagnostics| CompileResolvedPackageReviewsError::Projection {
                    package: key.clone(),
                    diagnostics,
                },
            )?;
        let obligations_bytes =
            retained_obligation_ledger_bytes(&obligations).ok_or_else(|| {
                CompileResolvedPackageReviewsError::RetainedObligationLedgerBudget {
                    package: key.clone(),
                    maximum_bytes: MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES,
                }
            })?;
        retained_obligation_ledger_total = reserve_retained_obligation_ledger_bytes(
            retained_obligation_ledger_total,
            obligations_bytes,
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
            selected_build_machine_identity,
            build_evaluation_usage,
            build_observation_summary,
            semantic_bindings,
            semantic_binding_candidates,
            generated_source_bundle: generated_source_bundle.clone(),
            projection,
            canonical_review_bytes,
            canonical_rows,
            obligations,
            obligation_results,
            comparison_rows,
        });
        if retain_checked_root && &key == closure.graph().root() {
            checked_root = Some(checked);
        }
    }
    verify_build_session_accounting(&reviews, evaluation_sponsor)?;
    Ok(CompiledPackageReviews {
        reviews: CompilerIssuedPackageReviewSet { reviews },
        checked_root,
    })
}
