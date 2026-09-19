//! Compile dependencies before their consumers; retain checked projections once.

use super::session_accounting::verify_build_session_accounting;
use compiler::CheckedCompileRequest;

use super::super::custody::{
    dependency_first_package_order, package_build_root, verify_selected_source_custody,
};
use super::super::rows::RetainedReviewRows;
use super::super::semantic_bindings::{
    SemanticBindingReviewCandidate, candidate_service_bindings, candidate_target_entry_binding,
};
use super::super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReview,
    CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
};
use super::ledger::{
    MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES, reserve_retained_obligation_ledger_bytes,
    retained_obligation_ledger_bytes,
};
use crate::declarations::PackageKey;
use crate::resolution::PackageCompilationScope;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use checked_interpreter::{BuildEvaluationSponsor, FilesystemSponsor};
use compiler::compile_to_checked;
use diagnostics::Diagnostic;
use package_compilation::{AcceptedSemanticBinding, PackageCompilationInputError};
use package_evidence::ledger::{ReconstructedPackageReview, reconstruct_package_review};
use package_evidence::record::PackagePolicyRepresentationProducerInstance;
use std::collections::BTreeMap;
use std::path::Path;

use super::{CandidateSourcePreparation, PreparedPackageSource};

pub(super) struct CompiledPackageReviews {
    pub(super) reviews: CompilerIssuedPackageReviewSet,
    pub(super) checked_root: Option<Box<compiler::CheckedCompilation>>,
}

/// Only the preliminary Discover pass may propose an already-checked
/// dependency's target entry schema. Source checkpoint reuse is independent:
/// final and Explicit passes consume exactly their supplied bindings.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetEntryDiscovery {
    Disabled,
    Dependencies,
}

/// Re-rooting already gives each dependency its own build activation and keeps
/// its build-only imports out of consumers. The current review and generated
/// bundle tables still have one slot per package, however: they cannot retain
/// two purpose/profile-specific results for the same package. Admit nested
/// activations only when that slot is unambiguous, before any build executes.
/// The graph has already rejected cycles over both kinds of dependency edge.
pub(super) fn validate_nested_build_activations(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    execution_profile: Option<target::TargetProfile>,
) -> Result<(), CompileResolvedPackageReviewsError> {
    use crate::declarations::DependencyPurpose;

    let graph = target_closure.source_closure().graph();
    let Some(nested) = graph.packages().iter().find(|package| {
        package.source().key() != graph.root()
            && package
                .dependencies()
                .iter()
                .any(|dependency| dependency.purpose() == DependencyPurpose::Build)
    }) else {
        return Ok(());
    };
    if execution_profile != Some(target_closure.target_profile()) {
        return Err(
            CompileResolvedPackageReviewsError::UnsupportedNestedBuildActivation {
                package: nested.source().key().clone(),
                reason: "cross-profile nested builds require separate execution-profile review and generated-source occurrences",
            },
        );
    }
    let mut purposes = vec![None; graph.packages().len()];
    let mut pending = vec![(graph.root(), DependencyPurpose::Product)];
    while let Some((package, purpose)) = pending.pop() {
        let Some(position) = graph.package_position(package) else {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch {
                package: package.clone(),
            });
        };
        if let Some(previous) = purposes[position] {
            if previous != purpose {
                return Err(
                    CompileResolvedPackageReviewsError::UnsupportedNestedBuildActivation {
                        package: package.clone(),
                        reason: "dual-purpose nested builds require separate build and product review and generated-source occurrences",
                    },
                );
            }
            continue;
        }
        purposes[position] = Some(purpose);
        for dependency in graph.packages()[position].dependencies() {
            // An ordinary dependency inherits the library's execution context;
            // a build edge belongs to that package's separate build activation.
            let selected_purpose = match dependency.purpose() {
                DependencyPurpose::Product => purpose,
                DependencyPurpose::Build => DependencyPurpose::Build,
            };
            pending.push((dependency.target(), selected_purpose));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_dependency_closure(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    execution_profile: Option<target::TargetProfile>,
    build_session_root: &Path,
    filesystem_sponsor: &FilesystemSponsor,
    evaluation_sponsor: &BuildEvaluationSponsor,
    semantic_bindings_by_consumer: &BTreeMap<PackageKey, Vec<AcceptedSemanticBinding>>,
    retained_root_entry: Option<&Path>,
    root_build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    discovery: TargetEntryDiscovery,
    preparation: &mut CandidateSourcePreparation,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let target = target_closure.target_profile().target_name();
    let mut reviews = Vec::<CompilerIssuedPackageReview>::with_capacity(closure.custodies().len());
    let mut review_positions: Vec<Option<usize>> = vec![None; closure.graph().packages().len()];
    let mut checked_root = None;
    // Each dependency's own checked compilation is retained as a component
    // candidate: a later consumer's `Independent` selection can only settle
    // against the description published from that compilation, and the
    // component-closure fence refuses any substitute. Cloning shares the
    // sealed program storage, so the same dependency can serve more than one
    // independent consumer.
    let mut component_candidates: BTreeMap<
        semantic_vocabulary::PackageKeyIdentity,
        (std::path::PathBuf, compiler::CheckedCompilation),
    > = BTreeMap::new();
    let mut retained_obligation_ledger_total = 0usize;
    // Sponsored build evaluations that ran inside a compile the
    // component-closure fence rejected: real session consumption no retained
    // review reports, reconciled beside the reviews at the session's end.
    let mut discarded_build_usages = Vec::<build_evaluation::BuildEvaluationUsage>::new();
    let mut retained_policy_canonical_total = 0usize;
    let mut target_entry_candidates = Vec::<SemanticBindingReviewCandidate>::new();
    for key in dependency_first_package_order(closure) {
        let scope = PackageCompilationScope::new(closure, &key);
        verify_selected_source_custody(&scope, PackageSourceVerificationPhase::BeforeCompilation)?;
        let custody = closure
            .custody(&key)
            .expect("validated source closure retains custody for every graph package");
        let inputs = scope.compilation_inputs().map_err(|errors| {
            CompileResolvedPackageReviewsError::CompilationInputs {
                package: key.clone(),
                errors,
            }
        })?;
        let mut semantic_bindings = semantic_bindings_by_consumer
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let mut proposed_entry = None;
        if discovery == TargetEntryDiscovery::Dependencies
            && inputs.root_role() == package_compilation::BuildDeclarationKind::Application
        {
            let candidates = target_entry_candidates
                .iter()
                .filter(|candidate| {
                    inputs.package_root(candidate.binding().package()).is_some()
                        && !semantic_bindings
                            .iter()
                            .any(|binding| binding.role() == candidate.binding().role())
                })
                .collect::<Vec<_>>();
            match candidates.as_slice() {
                [] => {}
                [candidate] => {
                    semantic_bindings.push(candidate.binding().clone());
                    proposed_entry = Some((*candidate).clone());
                }
                _ => {
                    return Err(
                        CompileResolvedPackageReviewsError::AmbiguousCandidateSemanticBinding {
                            consumer: key.clone(),
                            role: candidates[0].binding().role(),
                            candidate_count: candidates.len(),
                        },
                    );
                }
            }
        }
        let inputs = inputs
            .with_accepted_semantic_bindings(semantic_bindings)
            .map_err(
                |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                    package: key.clone(),
                    errors,
                },
            )?;
        let dependency_bundles = scope
            .packages()
            .iter()
            .filter(|dependency| *dependency != &key)
            .map(|dependency| {
                let review = closure
                    .graph()
                    .package_position(dependency)
                    .and_then(|position| review_positions[position])
                    .and_then(|position| reviews.get(position))
                    .ok_or(PackageCompilationInputError::MissingGeneratedSourceBundle {
                        package: dependency.identity(),
                    })?;
                let custody = closure.custody(dependency).ok_or(
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
        let dependency_source_instances = dependency_bundles
            .iter()
            .map(|bundle| (bundle.package(), bundle.source_consumption_commitment()))
            .collect::<BTreeMap<_, _>>();
        let inputs = inputs
            .with_complete_dependency_generated_sources(dependency_bundles)
            .map_err(
                |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                    package: key.clone(),
                    errors,
                },
            )?;
        let main_entry = custody.snapshot_root().join("main.omg");
        // Package review checks authored build-only packages through the compiler's
        // existing build entrance. Explicit application/check entries below retain
        // their own source requirement; no source or product exports are invented.
        let default_entry = if main_entry.is_file() {
            main_entry
        } else {
            custody.snapshot_root().join("build.omg")
        };
        let entry = if &key == closure.graph().root() {
            retained_root_entry.unwrap_or(&default_entry)
        } else {
            &default_entry
        };
        // Every package build activation runs against a captured immutable
        // snapshot of its own sealed source custody, never the live custody
        // root: build evaluation captures the inventory from the same
        // canonical metadata index the binding validated above, materializes
        // a fresh private Source root for this occurrence, and records the
        // inventory extent in the review's build observation. Only the root
        // may receive a caller-selected inventory and fixed output roster;
        // dependency builds retain their own complete inventories. Otherwise
        // the outputs a package build must complete are declared through
        // `builder.output.require`, registered during evaluation, and settled
        // against sealed staged custody before the result publishes; package
        // review has no fixed-output requirement of its own, and a roster
        // inferred here from the declaration would be a second roster source
        // that scoped execution forbids (a conditional branch omitting
        // `require` must not be reinterpreted as having checked the artifact).
        let build_snapshot = if &key == closure.graph().root() {
            root_build_snapshot.cloned()
        } else {
            None
        }
        .unwrap_or_else(|| {
            build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
        });
        // Every package compiles as the root of its own component view, so
        // any of them — not only the graph root — may author `Independent`
        // selections over dependencies this loop already compiled. The
        // discovery output carries those selections out of a compile the
        // component-closure fence rejects; `unanswered_inputs` keeps the
        // request's graph so the retried compile can attach the published
        // descriptions without rebuilding it.
        let unanswered_inputs = inputs.clone();
        let request = CheckedCompileRequest {
            build_execution_profile: execution_profile,
            build_dir: Some(
                package_build_root(build_session_root, &key, custody.resolution()).to_owned(),
            ),
            package_inputs: Some(inputs),
            filesystem_sponsor: Some(filesystem_sponsor.clone()),
            evaluation_sponsor: Some(evaluation_sponsor.clone()),
            build_snapshot: Some(build_snapshot.clone()),
            ..CheckedCompileRequest::new(entry, Some(target))
        };
        let position = closure
            .graph()
            .package_position(&key)
            .expect("reviewed package belongs to the validated graph");
        // A populated slot supplies only this package's binding-independent
        // parse frontier; the child still runs custody, build execution, and
        // checking, and the checkpoint is written back so a later pass or
        // candidate of this same closure prepares it once. A slot recorded
        // under another entry root belongs to a differently resolved package
        // at this position; it is replaced rather than consumed.
        let prepared = match preparation.slots[position].take() {
            Some(slot) if slot.entry_root == *entry => Some(slot.prepared),
            _ => None,
        };
        let mut retained = None;
        let mut component_discovery = compiler::IndependentComponentDiscovery::default();
        let checked = {
            let mut request = request;
            request.prepared_source_output = Some(&mut retained);
            request.independent_component_discovery_output = Some(&mut component_discovery);
            match prepared {
                Some(prepared) => prepared.compile_to_checked(request),
                None => {
                    preparation.fresh_preparations += 1;
                    compile_to_checked(request)
                }
            }
        };
        let checked = match checked {
            Ok(checked) => checked,
            Err(diagnostics) => {
                // The rejected attempt still consumed its sponsored build
                // evaluation; the session accounting counts that consumption
                // against the sponsor rather than losing it with the verdict.
                if let Some(usage) = component_discovery.evaluation_usage {
                    discarded_build_usages.push(usage);
                }
                let Some(inputs) = publish_independent_component_descriptions(
                    &key,
                    unanswered_inputs,
                    &component_discovery.selections,
                    &component_candidates,
                )?
                else {
                    return Err(CompileResolvedPackageReviewsError::Compilation {
                        package: key.clone(),
                        diagnostics,
                    });
                };
                // The descriptions a settled compile attached stay attached:
                // only a compile whose fence rejected unanswered selections
                // reaches here. One retry is the whole protocol — a selection
                // still unanswered after publication re-rejects at the same
                // fence, never as a fused edge.
                let mut request = CheckedCompileRequest {
                    build_execution_profile: execution_profile,
                    build_dir: Some(
                        package_build_root(build_session_root, &key, custody.resolution())
                            .to_owned(),
                    ),
                    package_inputs: Some(inputs),
                    filesystem_sponsor: Some(filesystem_sponsor.clone()),
                    evaluation_sponsor: Some(evaluation_sponsor.clone()),
                    build_snapshot: Some(build_snapshot),
                    ..CheckedCompileRequest::new(entry, Some(target))
                };
                request.prepared_source_output = Some(&mut retained);
                preparation.fresh_preparations += 1;
                compile_to_checked(request).map_err(|diagnostics| {
                    CompileResolvedPackageReviewsError::Compilation {
                        package: key.clone(),
                        diagnostics,
                    }
                })?
            }
        };
        preparation.slots[position] = retained.map(|prepared| PreparedPackageSource {
            entry_root: entry.to_path_buf(),
            prepared,
        });
        verify_selected_source_custody(&scope, PackageSourceVerificationPhase::AfterCompilation)?;
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
        let restricted_build_requests = checked.restricted_build_requests().to_vec();
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
        let ReconstructedPackageReview {
            projection,
            canonical_rows,
            ledger: obligations,
            results: obligation_results,
        } = reconstruct_package_review(&checked).map_err(|diagnostics| {
            CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics,
            }
        })?;
        if projection.package() != key.identity() {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch { package: key });
        }
        let (policy, policy_total) = super::policy::project(
            &checked,
            &key,
            target_closure.target_profile(),
            retained_policy_canonical_total,
        )?;
        // Dependency reviews precede consumers, so prior rows are available
        // for rejoining actual foreign by-value representation demands. Each
        // rejoin binds the producer review to the immutable source instance
        // this consumer compiled against through its retained dependency
        // bundle.
        policy
            .representation()
            .rejoin_foreign_demands(|identity| {
                let review = reviews
                    .iter()
                    .find(|review| review.key.identity() == identity)?;
                let expected_source_instance =
                    dependency_source_instances.get(&identity).copied()?;
                Some(PackagePolicyRepresentationProducerInstance {
                    policy: review.policy.representation(),
                    expected_source_instance,
                    reviewed_source_instance: review.source_consumption_commitment(),
                })
            })
            .map_err(
                |error| CompileResolvedPackageReviewsError::RepresentationAgreement {
                    consumer: key.clone(),
                    error,
                },
            )?;
        retained_policy_canonical_total = policy_total;
        let mut semantic_binding_candidates =
            candidate_service_bindings(&checked, &projection, &key)?;
        semantic_binding_candidates.extend(proposed_entry);
        if discovery == TargetEntryDiscovery::Dependencies {
            target_entry_candidates.extend(candidate_target_entry_binding(
                &checked,
                &key,
                target_closure.target_profile(),
            )?);
        }
        let canonical_review_bytes = projection.canonical_review_bytes().map_err(|error| {
            CompileResolvedPackageReviewsError::Encoding {
                package: key.clone(),
                error,
            }
        })?;
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
        let package_position = closure
            .graph()
            .package_position(&key)
            .expect("dependency-first order retains a position in the immutable graph");
        review_positions[package_position] = Some(reviews.len());
        reviews.push(CompilerIssuedPackageReview {
            key: key.clone(),
            resolution: custody.resolution().clone(),
            source_consumption_commitment,
            selected_build_machine_identity,
            build_evaluation_usage,
            build_observation_summary,
            restricted_build_requests,
            semantic_bindings,
            semantic_binding_candidates,
            generated_source_bundle: generated_source_bundle.clone(),
            projection,
            policy,
            canonical_review_bytes,
            canonical_rows: RetainedReviewRows(canonical_rows),
            obligations,
            obligation_results,
        });
        if &key == closure.graph().root() {
            if retained_root_entry.is_some() {
                checked_root = Some(Box::new(checked));
            }
        } else {
            component_candidates.insert(key.identity(), (entry.to_path_buf(), checked));
        }
    }
    verify_build_session_accounting(&reviews, &discarded_build_usages, evaluation_sponsor)?;
    Ok(CompiledPackageReviews {
        reviews: CompilerIssuedPackageReviewSet { reviews },
        checked_root,
    })
}

/// Publish the verified component descriptions a rejected compile's
/// discovered `Independent` selections name, and return the same inputs with
/// them attached. `Ok(None)` means the rejected compile authored no
/// answerable selection — its original diagnostics stand. A selection naming
/// the compiling package itself, or one whose provider carries no package
/// identity, has no dependency that could answer it. A selection naming a
/// package the closure never compiled, or whose checked compilation cannot
/// publish a description, fails the consumer's review directly instead of
/// retrying into the same fence.
fn publish_independent_component_descriptions(
    key: &PackageKey,
    inputs: package_compilation::PackageCompilationInputs,
    independent_component_selections: &[compiler::IndependentComponentSelection],
    component_candidates: &BTreeMap<
        semantic_vocabulary::PackageKeyIdentity,
        (std::path::PathBuf, compiler::CheckedCompilation),
    >,
) -> Result<Option<package_compilation::PackageCompilationInputs>, CompileResolvedPackageReviewsError>
{
    let mut described = std::collections::BTreeSet::new();
    let mut descriptions = Vec::new();
    for selection in independent_component_selections {
        let Some(component) = selection.component else {
            continue;
        };
        if component == key.identity() || !described.insert(component) {
            continue;
        }
        let Some((component_entry, component_checked)) = component_candidates.get(&component)
        else {
            return Err(CompileResolvedPackageReviewsError::Compilation {
                package: key.clone(),
                diagnostics: vec![Diagnostic::error(format!(
                    "an independent provider selection names dependency package `{}`, which this closure never compiled",
                    inputs.package_name(component).unwrap_or("<unnamed>"),
                ))],
            });
        };
        descriptions.push(
            compiler::published_independent_component_description(
                component_entry.clone(),
                component_checked.clone(),
                proof_admission::AdmissionProfile::default(),
            )
            .map_err(|diagnostics| {
                CompileResolvedPackageReviewsError::Compilation {
                    package: key.clone(),
                    diagnostics,
                }
            })?,
        );
    }
    if descriptions.is_empty() {
        return Ok(None);
    }
    inputs
        .with_independent_component_descriptions(descriptions)
        .map(Some)
        .map_err(
            |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                package: key.clone(),
                errors,
            },
        )
}
