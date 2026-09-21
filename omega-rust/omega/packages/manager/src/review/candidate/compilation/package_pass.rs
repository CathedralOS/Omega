//! Compile dependencies before their consumers, once per checked occurrence.
//!
//! Acquisition and the immutable parse checkpoint remain package-owned. Build
//! and product occurrences share those inputs, not checked output: each has its
//! own target, generated sources, bindings and review. The same source-roster
//! contract drives production, lock comparison and independent reconstruction.
//!
//! Named input assignments are reconciled across all incoming edges before
//! running authored code. A shared package/purpose/profile node may reuse equal
//! complete maps; unioning unequal maps would grant one requester another's
//! inputs. A separate tool package would still need capture, grant issuance and
//! checked generated-source handoff. Routing into these existing owners keeps
//! that enforcement in one execution path without making aliases new instances.

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
use crate::declarations::DependencyPurpose;
use crate::declarations::PackageKey;
use crate::lock::{
    PackageAcceptanceRow, PackageCheckedContext, PackageOccurrenceRoster, PackagePolicyAcceptance,
};
use crate::resolution::PackageCompilationScope;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::review::restricted_build_grants::RestrictedBuildCheckpoint;
use checked_interpreter::{BuildEvaluationSponsor, FilesystemSponsor};
use compiler::compile_to_checked;
use diagnostics::Diagnostic;
use package_compilation::{AcceptedSemanticBinding, PackageCompilationInputError};
use package_evidence::ledger::{ReconstructedPackageReview, reconstruct_package_review};
use package_evidence::record::{PackagePolicyRepresentationProducerInstance, PackagePolicyRowKind};
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

fn purpose_slot(purpose: DependencyPurpose) -> usize {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_dependency_closure(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    execution_profile: Option<target::TargetProfile>,
    roster: &PackageOccurrenceRoster,
    build_session_root: &Path,
    filesystem_sponsor: &FilesystemSponsor,
    evaluation_sponsor: &BuildEvaluationSponsor,
    semantic_bindings_by_consumer: &BTreeMap<
        (PackageKey, DependencyPurpose),
        Vec<AcceptedSemanticBinding>,
    >,
    retained_root_entry: Option<&Path>,
    root_build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    // A consuming compile supplies the accepted target's restricted-request
    // checkpoint so each occurrence's projected requests join its granted
    // meanings — bound to its exact checked context — before its own build
    // effect executes, before the review is retained, and before its
    // generated-source bundle hands off to a consumer.
    // `None` is audit-only observation: requests still project for review
    // material and the pass issues no grants.
    restricted_build_checkpoint: Option<&RestrictedBuildCheckpoint>,
    discovery: TargetEntryDiscovery,
    preparation: &mut CandidateSourcePreparation,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    if execution_profile.is_none()
        && let Some(coverage) = roster
            .coverages()
            .iter()
            .find(|coverage| coverage.purposes().contains(&DependencyPurpose::Build))
    {
        return Err(
            CompileResolvedPackageReviewsError::UnsupportedBuildActivation {
                package: coverage.package().clone(),
                reason: "build dependencies require an admitted build execution profile",
            },
        );
    }
    // Reconcile the entire invocation before any authored dependency runs.
    // Shared scheduling nodes may share equal assignments, never the union of
    // grants from different incoming edges (an unassigned edge is empty).
    let dependency_inputs = reconcile_build_inputs(target_closure, roster, root_build_snapshot)
        .map_err(
            |diagnostics| CompileResolvedPackageReviewsError::Compilation {
                package: closure.graph().root().clone(),
                diagnostics,
            },
        )?;
    let mut reviews = Vec::<CompilerIssuedPackageReview>::with_capacity(roster.occurrence_count());
    let mut review_positions = vec![[None::<usize>; 2]; closure.graph().packages().len()];
    let mut checked_root = None;
    // Each dependency's own checked compilation is retained as a component
    // candidate: a later consumer's `Independent` selection can only settle
    // against the description published from that compilation, and the
    // component-closure fence refuses any substitute. Cloning shares the
    // sealed program storage, so the same dependency can serve more than one
    // independent consumer.
    let mut component_candidates: BTreeMap<
        (semantic_vocabulary::PackageKeyIdentity, DependencyPurpose),
        (std::path::PathBuf, compiler::CheckedCompilation),
    > = BTreeMap::new();
    let mut retained_obligation_ledger_total = 0usize;
    // Sponsored build evaluations that ran inside a compile the
    // component-closure fence rejected: real session consumption no retained
    // review reports, reconciled beside the reviews at the session's end.
    let mut discarded_build_usages = Vec::<build_evaluation::BuildEvaluationUsage>::new();
    let mut retained_policy_canonical_total = 0usize;
    let mut target_entry_candidates =
        Vec::<(DependencyPurpose, SemanticBindingReviewCandidate)>::new();
    for key in dependency_first_package_order(closure) {
        let scope = PackageCompilationScope::new(closure, &key);
        verify_selected_source_custody(&scope, PackageSourceVerificationPhase::BeforeCompilation)?;
        let custody = closure
            .custody(&key)
            .expect("validated source closure retains custody for every graph package");
        let position = closure.graph().package_position(&key).ok_or_else(|| {
            CompileResolvedPackageReviewsError::IdentityMismatch {
                package: key.clone(),
            }
        })?;
        let purposes = roster.purposes(&key).ok_or_else(|| {
            CompileResolvedPackageReviewsError::IdentityMismatch {
                package: key.clone(),
            }
        })?;
        // One acquired source can produce two independently checked occurrences.
        // Every dependency package is complete before its consumer; role slots
        // retain the exact generated output and never alias equal-profile builds.
        for &purpose in purposes {
            let checked_target = if purpose.is_product() {
                target_closure.target_profile()
            } else {
                execution_profile.ok_or_else(|| {
                    CompileResolvedPackageReviewsError::UnsupportedBuildActivation {
                        package: key.clone(),
                        reason: "build dependencies require an admitted build execution profile",
                    }
                })?
            };
            let target = checked_target.target_name();
            let inputs = scope
                .compilation_inputs()
                .and_then(|inputs| inputs.with_compilation_purpose(purpose))
                .map_err(
                    |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                        package: key.clone(),
                        errors,
                    },
                )?;
            let mut semantic_bindings = semantic_bindings_by_consumer
                .get(&(key.clone(), purpose))
                .cloned()
                .unwrap_or_default();
            let mut proposed_entry = None;
            if discovery == TargetEntryDiscovery::Dependencies
                && inputs.root_role() == package_compilation::BuildDeclarationKind::Application
            {
                let candidates = target_entry_candidates
                    .iter()
                    .filter(|(candidate_purpose, candidate)| {
                        *candidate_purpose == purpose
                            && inputs.package_root(candidate.binding().package()).is_some()
                            && !semantic_bindings
                                .iter()
                                .any(|binding| binding.role() == candidate.binding().role())
                    })
                    .map(|(_, candidate)| candidate)
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
            let required_instances = inputs.required_dependency_source_instances();
            let mut dependency_bundles = Vec::with_capacity(required_instances.len());
            for dependency in scope
                .packages()
                .iter()
                .filter(|dependency| *dependency != &key)
            {
                for dependency_purpose in DependencyPurpose::ALL {
                    if !required_instances.contains(&(dependency.identity(), dependency_purpose)) {
                        continue;
                    }
                    let dependency_review = closure
                        .graph()
                        .package_position(dependency)
                        .and_then(|position| {
                            review_positions[position][purpose_slot(dependency_purpose)]
                        })
                        .and_then(|position| reviews.get(position));
                    let review = dependency_review.ok_or(
                        PackageCompilationInputError::MissingGeneratedSourceBundle {
                            package: dependency.identity(),
                        },
                    );
                    let review = review.map_err(|error| {
                        CompileResolvedPackageReviewsError::CompilationInputs {
                            package: key.clone(),
                            errors: vec![error],
                        }
                    })?;
                    let custody = closure
                        .custody(dependency)
                        .ok_or(PackageCompilationInputError::ForeignGeneratedSourceBundle {
                            package: dependency.identity(),
                        })
                        .map_err(
                            |error| CompileResolvedPackageReviewsError::CompilationInputs {
                                package: key.clone(),
                                errors: vec![error],
                            },
                        )?;
                    let bundle = review.generated_source_bundle();
                    if review.resolution() != custody.resolution()
                        || bundle.package() != dependency.identity()
                        || bundle.source_consumption_commitment()
                            != review.source_consumption_commitment()
                    {
                        return Err(CompileResolvedPackageReviewsError::CompilationInputs {
                            package: key.clone(),
                            errors: vec![
                        PackageCompilationInputError::GeneratedSourceBundleCustodyMismatch {
                            package: dependency.identity(),
                        }],
                        });
                    }
                    dependency_bundles.push(bundle.clone());
                }
            }
            let dependency_source_instances = dependency_bundles
                .iter()
                .map(|bundle| {
                    (
                        (bundle.package(), bundle.purpose()),
                        bundle.source_consumption_commitment(),
                    )
                })
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
            // may receive a caller-selected source inventory and fixed output
            // roster; dependency source inventories remain their own. Explicit
            // named inputs are separate grants selected for this occurrence. Otherwise
            // the outputs a package build must complete are declared through
            // `builder.output.require`, registered during evaluation, and settled
            // against sealed staged custody before the result publishes; package
            // review has no fixed-output requirement of its own, and a roster
            // inferred here from the declaration would be a second roster source
            // that scoped execution forbids (a conditional branch omitting
            // `require` must not be reinterpreted as having checked the artifact).
            let build_snapshot = if &key == closure.graph().root() {
                root_build_snapshot
                    .cloned()
                    .map(build_evaluation::BuildSnapshotRequest::without_dependency_inputs)
            } else {
                dependency_inputs[position][purpose_slot(purpose)]
                    .as_ref()
                    .map(|assignment| assignment.snapshot.clone())
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
                build_dir: Some(package_build_root(
                    build_session_root,
                    &key,
                    custody.resolution(),
                    purpose,
                )),
                package_inputs: Some(inputs),
                filesystem_sponsor: Some(filesystem_sponsor.clone()),
                evaluation_sponsor: Some(evaluation_sponsor.clone()),
                build_snapshot: Some(build_snapshot.clone()),
                // An armed checkpoint binds the admitted activation's
                // restricted requests to this occurrence's granted meanings
                // before each request's own build effect executes.
                restricted_build_grants: restricted_build_checkpoint.map(|checkpoint| {
                    Box::new(checkpoint.grants(
                        key.identity(),
                        PackageCheckedContext::new(purpose, checked_target, execution_profile),
                        closure.dependency_path(&key),
                    )) as Box<dyn compiler::RestrictedBuildGrants>
                }),
                collect_timings: preparation.collect_timings,
                ..CheckedCompileRequest::new(entry, Some(target))
            };
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
                        purpose,
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
                        build_dir: Some(package_build_root(
                            build_session_root,
                            &key,
                            custody.resolution(),
                            purpose,
                        )),
                        package_inputs: Some(inputs),
                        filesystem_sponsor: Some(filesystem_sponsor.clone()),
                        evaluation_sponsor: Some(evaluation_sponsor.clone()),
                        build_snapshot: Some(build_snapshot),
                        // The retried activation is the same checked
                        // occurrence: consent re-arms identically.
                        restricted_build_grants: restricted_build_checkpoint.map(|checkpoint| {
                            Box::new(checkpoint.grants(
                                key.identity(),
                                PackageCheckedContext::new(
                                    purpose,
                                    checked_target,
                                    execution_profile,
                                ),
                                closure.dependency_path(&key),
                            ))
                                as Box<dyn compiler::RestrictedBuildGrants>
                        }),
                        collect_timings: preparation.collect_timings,
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
            verify_selected_source_custody(
                &scope,
                PackageSourceVerificationPhase::AfterCompilation,
            )?;
            checked
                .verify_current_source_consumption()
                .map_err(|diagnostics| {
                    CompileResolvedPackageReviewsError::SourceConsumptionDrift {
                        package: key.clone(),
                        diagnostics,
                    }
                })?;
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
                checked_target,
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
                    let review = reviews.iter().find(|review| {
                        review.key.identity() == identity
                            && review.checked_context().purpose() == purpose
                    })?;
                    let expected_source_instance = dependency_source_instances
                        .get(&(identity, purpose))
                        .copied()?;
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
                target_entry_candidates.extend(
                    candidate_target_entry_binding(&checked, &key, checked_target)?
                        .map(|candidate| (purpose, candidate)),
                );
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
            let review = CompilerIssuedPackageReview {
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
            };
            // A consuming compile joins each occurrence's projected restricted
            // requests against its retained consent here — before the review is
            // retained and before the generated-source bundle can hand off to a
            // dependent activation — rather than at the consuming operation's
            // boundary after the whole closure compiled. An ungranted
            // occurrence rejects with its pending request meanings attached.
            if let Some(checkpoint) = restricted_build_checkpoint {
                let projected =
                    PackagePolicyAcceptance::from_policy(review.policy()).map_err(|error| {
                        CompileResolvedPackageReviewsError::Projection {
                            package: key.clone(),
                            diagnostics: vec![Diagnostic::error(error.to_string())],
                        }
                    })?;
                let ungranted = checkpoint.ungranted_requests(
                    review.key().identity(),
                    review.checked_context(),
                    closure.dependency_path(review.key()),
                    projected
                        .rows()
                        .iter()
                        .filter(|row| row.kind() == PackagePolicyRowKind::RestrictedBuildRequest)
                        .map(PackageAcceptanceRow::canonical_text),
                );
                if !ungranted.is_empty() {
                    return Err(
                        CompileResolvedPackageReviewsError::UngrantedRestrictedBuildRequests {
                            package: key,
                            ungranted,
                        },
                    );
                }
            }
            review_positions[position][purpose_slot(purpose)] = Some(reviews.len());
            reviews.push(review);
            if &key == closure.graph().root() {
                if retained_root_entry.is_some() {
                    checked_root = Some(Box::new(checked));
                }
            } else {
                component_candidates
                    .insert((key.identity(), purpose), (entry.to_path_buf(), checked));
            }
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
    purpose: DependencyPurpose,
    inputs: package_compilation::PackageCompilationInputs,
    independent_component_selections: &[compiler::IndependentComponentSelection],
    component_candidates: &BTreeMap<
        (semantic_vocabulary::PackageKeyIdentity, DependencyPurpose),
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
        let Some((component_entry, component_checked)) =
            component_candidates.get(&(component, purpose))
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

struct OccurrenceBuildInputs {
    incoming: package_compilation::BuildDependencyOccurrence,
    snapshot: build_evaluation::BuildSnapshotRequest,
}

/// The spec's scheduling node is package/purpose/profiles, not import alias.
/// Equal edge assignments can reuse that node; incompatible complete maps
/// reject before execution rather than granting their union or multiplying
/// package identity. Product edges propagate their requester's purposes, as
/// in PackageOccurrenceRoster; build edges always select the build purpose.
fn reconcile_build_inputs(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    roster: &PackageOccurrenceRoster,
    request: Option<&build_evaluation::BuildSnapshotRequest>,
) -> Result<Vec<[Option<OccurrenceBuildInputs>; 2]>, Vec<Diagnostic>> {
    use build_evaluation::BuildSnapshotRequest;
    use package_compilation::BuildDependencyOccurrence;

    let graph = target_closure.source_closure().graph();
    let mut assigned: Vec<[Option<OccurrenceBuildInputs>; 2]> =
        graph.packages().iter().map(|_| [None, None]).collect();
    let Some(request) = request else {
        return Ok(assigned);
    };
    for (occurrence, _) in request.dependency_inputs() {
        let exists = graph.packages().iter().any(|node| {
            node.source().key().identity() == occurrence.requester()
                && node.dependencies().iter().any(|edge| {
                    edge.purpose() == occurrence.purpose()
                        && edge.alias().as_str() == occurrence.alias()
                        && edge.target().identity() == occurrence.target()
                })
        });
        if !exists {
            return Err(vec![Diagnostic::error(format!(
                "named build inputs are assigned to a dependency occurrence that does not exist: {occurrence:?}"
            ))]);
        }
    }
    let empty = BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>());
    for node in graph.packages() {
        let requester = node.source().key();
        for edge in node.dependencies() {
            let occurrence = BuildDependencyOccurrence::new(
                requester.identity(),
                edge.purpose(),
                edge.alias().as_str(),
                edge.target().identity(),
            );
            let slots = request
                .dependency_input_slots(&occurrence)
                .unwrap_or_else(|| empty.inputs());
            let position = graph
                .package_position(edge.target())
                .expect("validated graph edge target");
            for &requester_purpose in roster
                .purposes(requester)
                .expect("closed occurrence roster")
            {
                let purpose = if edge.purpose().is_product() {
                    requester_purpose
                } else {
                    DependencyPurpose::Build
                };
                let assignment = &mut assigned[position][purpose_slot(purpose)];
                if let Some(previous) = assignment {
                    if previous.snapshot.inputs() != slots {
                        return Err(vec![Diagnostic::error(format!(
                            "conflicting named build input assignments for shared {:?} activation of `{}`: {:?} and {:?}; assignments must match exactly, including unassigned inputs",
                            purpose,
                            edge.target().name().as_str(),
                            previous.incoming,
                            occurrence,
                        ))]);
                    }
                } else {
                    *assignment = Some(OccurrenceBuildInputs {
                        incoming: occurrence.clone(),
                        snapshot: empty.clone().with_inputs(
                            slots
                                .iter()
                                .map(|(name, input)| (name.clone(), input.clone())),
                        )?,
                    });
                }
            }
        }
    }
    Ok(assigned)
}
