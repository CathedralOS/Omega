//! Compile accepted package inputs without reconstructing package review.
//!
//! The 2026-10-03 owner ruling makes review an explicit acceptance operation.
//! Matching source and compiler identities select retained compilation inputs;
//! they do not prove generated code, supply host grants, or waive Psi checking.

mod identity;
pub(crate) use identity::compiler_input_identity;

use crate::build_session::{
    PackageBuildSession, PackageSourcePreparation, PreparedPackageSource,
    dependency_first_package_order, package_build_root, publish_checked_component,
    reconcile_build_inputs,
};
use crate::declarations::{DependencyPurpose, PackageKey};
use crate::lock::{PackageCheckedContext, PackageLockTarget, PackageOccurrenceRoster};
use crate::resolution::PackageCompilationScope;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ExactTargetPackageSourceClosure,
};
use crate::review::{CompileResolvedPackageReviewsError, RestrictedBuildCheckpoint};
use compilation_products::PreparedPackageBuild;
use compiler::{CheckedCompilation, CheckedCompileRequest};
use diagnostics::Diagnostic;
use package_compilation::{IndependentComponentDescription, PackageGeneratedSourceBundle};
use std::path::Path;

type Error = CompileResolvedPackageReviewsError;

/// Inputs for ordinary compilation, distinct from a review candidate. An absent
/// lock supplies no accepted bindings or restricted grants. It never triggers
/// an inline audit or creates acceptance as a side effect of compilation.
pub(super) struct AcceptedCompilationRequest<'request> {
    pub closure: &'request ExactTargetPackageSourceClosure<'request>,
    pub accepted: Option<&'request PackageLockTarget>,
    pub entry: &'request Path,
    pub build_root: &'request Path,
    pub snapshot: Option<&'request build_evaluation::BuildSnapshotRequest>,
    pub verify: bool,
    pub timings: bool,
}

struct OccurrenceProducts {
    generated: PackageGeneratedSourceBundle,
    /// Only an actual independent-product request consumes this continuation.
    /// Fused consumers check the dependency modules they import themselves.
    prepared: Option<PreparedPackageBuild>,
    description: Option<IndependentComponentDescription>,
}

fn purpose_slot(purpose: DependencyPurpose) -> usize {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}

fn failure(package: &PackageKey, message: impl Into<String>) -> Error {
    Error::Compilation {
        package: package.clone(),
        diagnostics: vec![Diagnostic::error(message.into())],
    }
}

/// Validate acceptance, execute required Builds dependency-first, then check the
/// consumer once. No review projection or discovery/bound compilation is called.
pub(super) fn compile_accepted_project(
    request: AcceptedCompilationRequest<'_>,
    preparation: &mut PackageSourcePreparation,
) -> Result<CheckedCompilation, Error> {
    let root = request.closure.source_closure().graph().root();
    let compiler_identity = compiler_input_identity().map_err(|error| failure(root, error))?;
    if let Some(accepted) = request.accepted {
        identity::require_current_acceptance(request.closure, accepted, compiler_identity)
            .map_err(|error| failure(root, error))?;
    }
    preparation.size_for(request.closure.source_closure());
    let session = PackageBuildSession::create(request.build_root)?;
    let result = compile_occurrences(&request, &session, preparation);
    // Loaded Rust identity is immutable; the external library is not. Reject
    // edits during compilation instead of labeling mixed inputs as accepted.
    let result = result.and_then(|checked| {
        let current = compiler_input_identity().map_err(|error| failure(root, error))?;
        if current != compiler_identity {
            return Err(failure(
                root,
                "compiler library changed during compilation; run `omega update` for fresh review",
            ));
        }
        Ok(checked)
    });
    session.dispose(result)
}

fn compile_occurrences(
    request: &AcceptedCompilationRequest<'_>,
    session: &PackageBuildSession,
    preparation: &mut PackageSourcePreparation,
) -> Result<CheckedCompilation, Error> {
    let closure = request.closure.source_closure();
    let graph = closure.graph();
    let root = graph.root();
    let source = CanonicalSourceClosureSubject::from_resolved(
        request.closure,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .map_err(|error| failure(root, error.to_string()))?;
    let roster = PackageOccurrenceRoster::derive(&source)
        .map_err(|error| failure(root, error.to_string()))?;
    let snapshots = reconcile_build_inputs(request.closure, &roster, request.snapshot).map_err(
        |diagnostics| Error::Compilation {
            package: root.clone(),
            diagnostics,
        },
    )?;
    let execution_profile = target::TargetProfile::host_if_supported();
    let checkpoint = if request.verify {
        None
    } else {
        request.accepted.map(RestrictedBuildCheckpoint::derive)
    };
    let mut products: Vec<[Option<OccurrenceProducts>; 2]> =
        graph.packages().iter().map(|_| [None, None]).collect();
    let mut checked_root = None;

    for key in dependency_first_package_order(closure) {
        let position = graph
            .package_position(&key)
            .ok_or_else(|| failure(&key, "resolved package has no graph position"))?;
        let custody = closure
            .custody(&key)
            .ok_or_else(|| failure(&key, "resolved package has no source custody"))?;
        let purposes = roster
            .purposes(&key)
            .ok_or_else(|| failure(&key, "resolved package has no occurrence roster"))?;
        for &purpose in purposes {
            let started = std::time::Instant::now();
            let target = if purpose.is_product() {
                request.closure.target_profile()
            } else {
                execution_profile.ok_or_else(|| {
                    failure(
                        &key,
                        "Build dependencies require an admitted execution profile",
                    )
                })?
            };
            let context = PackageCheckedContext::new(purpose, target, execution_profile);
            let bindings = request
                .accepted
                .and_then(|accepted| {
                    accepted.occurrences().iter().find(|occurrence| {
                        occurrence.acceptance().package() == key.identity()
                            && occurrence.context() == context
                    })
                })
                .and_then(|occurrence| occurrence.compilation())
                .map(|compilation| compilation.semantic_bindings().to_vec())
                .unwrap_or_default();
            let mut inputs = PackageCompilationScope::new(closure, &key)
                .compilation_inputs()
                .and_then(|inputs| inputs.with_compilation_purpose(purpose))
                .and_then(|inputs| inputs.with_accepted_semantic_bindings(bindings))
                .map_err(|errors| Error::CompilationInputs {
                    package: key.clone(),
                    errors,
                })?;
            let required = inputs.required_dependency_source_instances();
            let mut generated = Vec::with_capacity(required.len());
            for (dependency_position, package) in graph.packages().iter().enumerate() {
                for dependency_purpose in DependencyPurpose::ALL {
                    if !required.contains(&(package.source().key().identity(), dependency_purpose))
                    {
                        continue;
                    }
                    let product = products[dependency_position][purpose_slot(dependency_purpose)]
                        .as_ref()
                        .ok_or_else(|| {
                            failure(
                                &key,
                                "dependency Build output is missing for its required purpose",
                            )
                        })?;
                    generated.push(product.generated.clone());
                }
            }
            inputs = inputs
                .with_complete_dependency_generated_sources(generated)
                .map_err(|errors| Error::CompilationInputs {
                    package: key.clone(),
                    errors,
                })?;
            let is_root = &key == root && purpose.is_product();
            let entry = if is_root {
                request.entry.to_path_buf()
            } else {
                let main = custody.snapshot_root().join("main.omg");
                if main.is_file() {
                    main
                } else {
                    custody.snapshot_root().join("build.omg")
                }
            };
            let snapshot = if is_root {
                request
                    .snapshot
                    .cloned()
                    .map(build_evaluation::BuildSnapshotRequest::without_dependency_inputs)
            } else {
                snapshots[position][purpose_slot(purpose)]
                    .as_ref()
                    .map(|assignment| assignment.snapshot.clone())
            }
            .unwrap_or_else(|| {
                build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
            });
            let prepared_source = match preparation.slots[position].take() {
                Some(slot) if slot.entry_root == entry => Some(slot.prepared),
                _ => None,
            };
            let outcome = compiler::prepare_package_build(
                CheckedCompileRequest {
                    prepared_source,
                    retain_prepared_source: true,
                    package_inputs: Some(inputs),
                    build_execution_profile: execution_profile,
                    build_dir: Some(package_build_root(
                        session.root(),
                        &key,
                        custody.resolution(),
                        purpose,
                    )),
                    filesystem_sponsor: Some(session.filesystem_sponsor().clone()),
                    evaluation_sponsor: Some(session.evaluation_sponsor().clone()),
                    build_snapshot: Some(snapshot),
                    restricted_build_grants: checkpoint.as_ref().map(|checkpoint| {
                        Box::new(checkpoint.grants(
                            key.identity(),
                            context,
                            closure.dependency_path(&key),
                        )) as Box<dyn compiler::RestrictedBuildGrants>
                    }),
                    collect_timings: request.timings,
                    ..CheckedCompileRequest::new(&entry, Some(target.target_name()))
                },
                Some(&preparation.pure_build_compilation),
            );
            preparation.slots[position] =
                outcome
                    .prepared_source
                    .map(|prepared| PreparedPackageSource {
                        entry_root: entry,
                        prepared,
                    });
            let prepared = outcome.result.map_err(|diagnostics| Error::Compilation {
                package: key.clone(),
                diagnostics,
            })?;
            let descriptions = selected_component_descriptions(
                &key,
                purpose,
                prepared.build().independent_components().selections,
                graph,
                &mut products,
            )?;
            let prepared = prepared
                .with_independent_component_descriptions(descriptions)
                .map_err(|errors| Error::CompilationInputs {
                    package: key.clone(),
                    errors,
                })?;
            if is_root {
                let outcome = compiler::check_prepared_package_build(prepared);
                checked_root = Some(outcome.result.map_err(|diagnostics| Error::Compilation {
                    package: key.clone(),
                    diagnostics,
                })?);
            } else {
                products[position][purpose_slot(purpose)] = Some(OccurrenceProducts {
                    generated: prepared.generated_source_bundle().clone(),
                    prepared: Some(prepared),
                    description: None,
                });
            }
            if request.timings {
                eprintln!(
                    "omega_compile_timings stage=package {} purpose={purpose:?} micros={}",
                    key.name().as_str(),
                    started.elapsed().as_micros()
                );
            }
        }
    }
    checked_root.ok_or_else(|| failure(root, "ordinary compilation produced no checked root"))
}

fn selected_component_descriptions(
    consumer: &PackageKey,
    purpose: DependencyPurpose,
    selections: Vec<compilation_products::IndependentComponentSelection>,
    graph: &crate::resolution::graph::ResolvedPackageClosure,
    products: &mut [[Option<OccurrenceProducts>; 2]],
) -> Result<Vec<IndependentComponentDescription>, Error> {
    let mut descriptions = Vec::new();
    for selection in selections {
        let Some(component) = selection.component else {
            continue;
        };
        if component == consumer.identity()
            || descriptions
                .iter()
                .any(|description: &IndependentComponentDescription| {
                    description.package() == component
                })
        {
            continue;
        }
        let position = graph
            .packages()
            .iter()
            .position(|package| package.source().key().identity() == component)
            .ok_or_else(|| {
                failure(
                    consumer,
                    "independent component is outside the resolved package closure",
                )
            })?;
        let products = products[position][purpose_slot(purpose)]
            .as_mut()
            .ok_or_else(|| {
                failure(
                    consumer,
                    "independent component has no prepared Build for this purpose",
                )
            })?;
        if products.description.is_none() {
            let prepared = products.prepared.take().ok_or_else(|| {
                failure(
                    consumer,
                    "independent component continuation was already consumed",
                )
            })?;
            let checked = compiler::check_prepared_package_build(prepared)
                .result
                .map_err(|diagnostics| Error::Compilation {
                    package: consumer.clone(),
                    diagnostics,
                })?;
            products.description =
                Some(publish_checked_component(checked).map_err(|diagnostics| {
                    Error::Compilation {
                        package: consumer.clone(),
                        diagnostics,
                    }
                })?);
        }
        descriptions.extend(products.description.iter().cloned());
    }
    Ok(descriptions)
}
