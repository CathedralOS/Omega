//! Compile accepted package inputs without reconstructing package review.
//!
//! The 2026-10-03 owner ruling makes review an explicit acceptance operation.
//! Matching source and compiler identities select retained compilation inputs;
//! they do not prove generated code, supply host grants, or waive Psi checking.

mod identity;
pub(crate) use identity::compiler_input_identity;

use crate::build_session::{
    PackageBuildSession, dependency_first_package_order, reconcile_build_inputs,
};
use crate::declarations::{PackageKey, purpose_slot};
use crate::lock::{PackageCheckedContext, PackageLockTarget, PackageOccurrenceRoster};
use crate::resolution::PackageCompilationScope;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ExactTargetPackageSourceClosure,
};
use crate::review::{CompileResolvedPackageReviewsError, RestrictedBuildCheckpoint};
use compiler::{
    CheckedCompileRequest, CompilationOutput, CompilationProduct, PackageCompilationRequest,
};
use diagnostics::Diagnostic;
use std::path::Path;

type Error = CompileResolvedPackageReviewsError;

/// Inputs for ordinary compilation, distinct from a review candidate. An absent
/// lock supplies no accepted bindings or restricted grants. It never triggers
/// an inline audit or creates acceptance as a side effect of compilation.
pub(super) struct AcceptedCompilationRequest<'request> {
    pub closure: ExactTargetPackageSourceClosure<'request>,
    pub accepted: Option<&'request PackageLockTarget>,
    pub entry: &'request Path,
    pub snapshot: Option<&'request build_evaluation::BuildSnapshotRequest>,
    pub verify: bool,
    pub timings: bool,
    pub product: CompilationProduct,
}

fn failure(package: &PackageKey, message: impl Into<String>) -> Error {
    Error::Compilation {
        package: package.clone(),
        diagnostics: vec![Diagnostic::error(message.into())],
    }
}

/// Validate acceptance and submit a dependency-ordered custody/consent plan.
/// The compiler owns Build execution and Product completion. No review
/// projection or discovery/bound compilation is called.
///
/// The caller owns the invocation: one session covers every target of a batch
/// request instead of each target entering its own checked activation with a
/// fresh grant budget and staging owner. Disposal stays with that owner.
pub(super) fn compile_accepted_project(
    request: AcceptedCompilationRequest<'_>,
    session: &mut PackageBuildSession,
) -> Result<CompilationOutput, Error> {
    let root = request.closure.source_closure().graph().root().clone();
    compile_accepted_projects(vec![request], session)
        .into_iter()
        .next()
        .ok_or_else(|| failure(&root, "compiler lost the requested package outcome"))?
}

/// Submit every accepted target before completing Product source. Target-local
/// input failures remain ordered outcomes; they do not discard valid siblings.
pub(super) fn compile_accepted_projects(
    requests: Vec<AcceptedCompilationRequest<'_>>,
    session: &mut PackageBuildSession,
) -> Vec<Result<CompilationOutput, Error>> {
    let mut outcomes: Vec<_> = (0..requests.len()).map(|_| None).collect();
    let roots: Vec<_> = requests
        .iter()
        .map(|request| request.closure.source_closure().graph().root().clone())
        .collect();
    let mut positions = Vec::new();
    let mut plans = Vec::new();
    for (position, request) in requests.into_iter().enumerate() {
        match prepare_accepted_project(request, session) {
            Ok((plan, packages, identity)) => {
                positions.push((position, packages, identity));
                plans.push(plan);
            }
            Err(error) => outcomes[position] = Some(Err(error)),
        }
    }
    let checked = session.compiler.compile_packages(plans);
    // Rust identity is immutable, but the external library can change during
    // the batch. Never label mixed compiler inputs as accepted.
    let current_identity = compiler_input_identity();
    for ((position, packages, identity), checked) in positions.into_iter().zip(checked) {
        let root = &roots[position];
        let result = checked
            .map_err(|error| Error::Compilation {
                package: packages.get(error.occurrence).unwrap_or(root).clone(),
                diagnostics: error.failure.into_diagnostics(),
            })
            .and_then(|checked| {
                let current = current_identity
                    .as_ref()
                    .map_err(|error| failure(root, error.clone()))?;
                if *current != identity {
                    return Err(failure(
                        root,
                        "compiler library changed during compilation; run `omega update` for fresh review",
                    ));
                }
                Ok(checked)
            });
        outcomes[position] = Some(result);
    }
    outcomes
        .into_iter()
        .zip(roots)
        .map(|(outcome, root)| {
            outcome.unwrap_or_else(|| Err(failure(&root, "compiler lost a package target outcome")))
        })
        .collect()
}

fn prepare_accepted_project(
    request: AcceptedCompilationRequest<'_>,
    session: &mut PackageBuildSession,
) -> Result<(PackageCompilationRequest, Vec<PackageKey>, [u8; 32]), Error> {
    let root = request.closure.source_closure().graph().root();
    let compiler_identity = compiler_input_identity().map_err(|error| failure(root, error))?;
    if let Some(accepted) = request.accepted {
        identity::require_current_acceptance(&request.closure, accepted, compiler_identity)
            .map_err(|error| failure(root, error))?;
    }
    let terminal_permissions = if matches!(
        request.product,
        CompilationProduct::Native(_) | CompilationProduct::Terminal(_)
    ) {
        request.accepted.map(|accepted| {
            accepted.occurrences().iter().find(|occurrence| {
                occurrence.acceptance().package() == root.identity()
                    && occurrence.context().purpose().is_product()
                    && occurrence.context().target() == request.closure.target_profile()
            }).map(|occurrence| occurrence.acceptance().terminal_permissions())
                .ok_or_else(|| failure(root, "locked target has no accepted root Product occurrence; run omega update"))
        }).transpose()?
    } else {
        None
    };
    let (occurrences, packages) = prepare_occurrences(&request, session)?;
    let plan = PackageCompilationRequest {
        occurrences,
        product: request.product,
        terminal_permissions,
    };
    Ok((plan, packages, compiler_identity))
}

fn prepare_occurrences(
    request: &AcceptedCompilationRequest<'_>,
    session: &mut PackageBuildSession,
) -> Result<(Vec<CheckedCompileRequest>, Vec<PackageKey>), Error> {
    let closure = request.closure.source_closure();
    let graph = closure.graph();
    let root = graph.root();
    let source = CanonicalSourceClosureSubject::from_resolved(
        &request.closure,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .map_err(|error| failure(root, error.to_string()))?;
    let roster = PackageOccurrenceRoster::derive(&source)
        .map_err(|error| failure(root, error.to_string()))?;
    let snapshots = reconcile_build_inputs(&request.closure, &roster, request.snapshot).map_err(
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
    let mut occurrences = Vec::new();
    let mut packages = Vec::new();

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
        let mut root_binding = None;
        for &purpose in purposes {
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
            let inputs = PackageCompilationScope::new(closure, &key)
                .compilation_inputs_reusing(&mut root_binding)
                .and_then(|inputs| inputs.with_compilation_purpose(purpose))
                .and_then(|inputs| inputs.with_accepted_semantic_bindings(bindings))
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
            let compile_request = CheckedCompileRequest {
                package_inputs: Some(inputs),
                build_execution_profile: execution_profile,
                build_dir: Some(session.create_occurrence_directory(
                    &key,
                    custody.resolution(),
                    purpose,
                )?),
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
                optimization_rollback: request.product.optimization_rollback(),
                ..CheckedCompileRequest::new(&entry, Some(target.target_name()))
            };
            occurrences.push(compile_request);
            packages.push(key.clone());
        }
    }
    Ok((occurrences, packages))
}
