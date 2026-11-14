/*

Execute a dependency-first package review pass (`execute_pass`).

`package_manager` plans the review occurrences and decides which cached review
records are reusable; this module drives the compiler over the occurrences that
still need work (`round.admit_occurrence`).

Why `execute_pass` loops over rounds (`fresh_components`, `recovered`, `pending_builds`):
A cached review record for an independent component dependency is sufficient to
report that dependency's own policy findings, but it may not retain the
in-memory `IndependentComponentDescription` needed when a downstream consumer
has to be compiled in the same pass. When `component_descriptions` hits a
cached dependency that lacks its description, it returns
`CompileResolvedPackageReviewsError::ComponentDescriptionNotRetained`. We then:
1. Stash the waiting consumer's prepared `PackageBuild` in `pending_builds` so
   we do not re-prepare its product from scratch on the next round (verified by
   matching `entry`, `snapshot`, and `package_inputs()`).
2. Keep the already-completed review results from the interrupted round in
   `recovered` (`round.into_retained()`).
3. Insert `(component, purpose)` into `fresh_components` and start a new round
   so `plan.begin_round` forces that component to compile and publish its
   description before the waiting consumer resumes.

When a reviewed occurrence produces a cacheable result (`round.retain`), we
attempt `source.publish_component().ok()` before `cache_write.store(component)`.
A component-publication failure there is left as `None` (a cache miss for future
component consumers) because a package that is never consumed as an independent
component does not need a component description; if a consumer later demands it,
`component_descriptions` will surface the exact compilation diagnostic.

*/

use compiler::PackageBuild;
use diagnostics::Diagnostic;
use package_manager::PackageSourceBindings;
use package_manager::review::{
    CompileResolvedPackageReviewsError, PackageReviewComponent, PackageReviewPass,
    PackageReviewRound, PackageReviewWork, RetainedPackageReviews,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

struct PendingReviewBuild {
    build: PackageBuild,
    entry: PathBuf,
    snapshot: Option<package_compilation::BuildSnapshotRequest>,
}

pub(crate) fn execute_pass(
    plan: PackageReviewPass<'_>,
    compiler: &mut compiler::CompilationSession,
    source_bindings: &mut PackageSourceBindings,
    builds: &mut [compiler::CompletedBuild],
) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
    let mut fresh_components = BTreeSet::new();
    let mut recovered = Vec::new();
    let mut pending_builds = Vec::new();
    loop {
        let mut round = plan.begin_round(source_bindings, &fresh_components, &recovered);
        let result = execute_round(&plan, &mut round, compiler, &mut pending_builds, builds);
        match result {
            Ok(()) => return round.finish(),
            Err(error) => {
                let mut accounting = round.source_completion_accounting();
                for prior in &recovered {
                    accounting.append(prior.source_completion_accounting());
                }
                let error = error.with_source_completion_accounting(accounting);
                recovered.push(round.into_retained());
                let retry = match &error.cause {
                    package_manager::review::CompileResolvedPackageReviewCause::ComponentDescriptionNotRetained { component, purpose, .. } =>
                        fresh_components.insert((*component, *purpose)),
                    _ => false,
                };
                if !retry {
                    return Err(error);
                }
            }
        }
    }
}

fn execute_round<'pass>(
    plan: &'pass PackageReviewPass<'_>,
    round: &mut PackageReviewRound<'pass>,
    compiler: &mut compiler::CompilationSession,
    pending_builds: &mut Vec<PendingReviewBuild>,
    builds: &mut [compiler::CompletedBuild],
) -> Result<(), CompileResolvedPackageReviewsError> {
    for occurrence in plan.occurrences() {
        let Some(work) = round.admit_occurrence(occurrence)? else {
            continue;
        };
        complete_occurrence(work, round, compiler, pending_builds, builds)?;
    }
    Ok(())
}

fn complete_occurrence(
    work: PackageReviewWork<'_>,
    round: &mut PackageReviewRound<'_>,
    compiler: &mut compiler::CompilationSession,
    pending_builds: &mut Vec<PendingReviewBuild>,
    builds: &mut [compiler::CompletedBuild],
) -> Result<(), CompileResolvedPackageReviewsError> {
    let PackageReviewWork {
        request,
        completion,
        maximum_policy_canonical_bytes,
    } = work;
    let inputs = &request.inputs;
    let entry = request.entry.clone();
    let snapshot = request.execution.snapshot.clone();
    let pending = pending_builds
        .iter()
        .position(|pending| {
            pending.build.package_inputs().root() == inputs.root()
                && pending.build.package_inputs().compilation_purpose()
                    == inputs.compilation_purpose()
        })
        .map(|position| pending_builds.swap_remove(position));
    let pending = pending.filter(|pending| {
        pending.entry == entry
            && pending.snapshot == snapshot
            && pending.build.package_inputs() == inputs
    });
    let build = match pending {
        Some(pending) => pending.build,
        None => {
            let source = builds
                .iter_mut()
                .find(|source| source.package_inputs().root() == request.inputs.root())
                .ok_or_else(|| {
                    completion.compilation_failure(
                        vec![Diagnostic::error(
                            "review request has no settled source Build",
                        )]
                        .into(),
                    )
                })?;
            source
                .require_execution_contract(&request.execution)
                .map_err(|failure| completion.compilation_failure(failure))?;
            compiler
                .prepare_package_product(
                    source,
                    request.target,
                    request.inputs,
                    compiler::PackageProductOptions::default(),
                )
                .map_err(|failure| completion.compilation_failure(failure))?
        }
    };

    let accounting = build.source_completion_accounting().clone();
    let descriptions = component_descriptions(&build, &completion, round);
    let descriptions = match descriptions {
        Ok(descriptions) => descriptions,
        Err(error) => {
            if matches!(
                &error.cause,
                package_manager::review::CompileResolvedPackageReviewCause::ComponentDescriptionNotRetained { .. }
            ) {
                pending_builds.push(PendingReviewBuild {
                    build,
                    entry,
                    snapshot,
                });
            }
            return Err(error.with_source_completion_accounting(accounting));
        }
    };
    let build = build
        .with_independent_component_descriptions(descriptions)
        .map_err(|errors| {
            completion
                .input_failure(errors)
                .with_source_completion_accounting(accounting)
        })?;
    let inputs = build.package_inputs().clone();
    let product = compiler
        .review_package_build(build, maximum_policy_canonical_bytes)
        .map_err(|failure| completion.review_failure(failure))?;
    if let Some(cache_write) = round.retain(completion, product, inputs)? {
        let component = match cache_write.component_source() {
            Some(source) => source.publish_component().ok(),
            None => None,
        };
        cache_write.store(component);
    }
    Ok(())
}

fn component_descriptions(
    build: &PackageBuild,
    completion: &package_manager::review::PackageReviewCompletion<'_>,
    round: &PackageReviewRound<'_>,
) -> Result<
    Vec<package_compilation::IndependentComponentDescription>,
    CompileResolvedPackageReviewsError,
> {
    let inputs = build.package_inputs();
    let mut descriptions: Vec<_> = inputs
        .independent_component_descriptions()
        .cloned()
        .collect();
    for component in build.required_component_packages() {
        if descriptions
            .iter()
            .any(|description| description.package() == component)
        {
            continue;
        }
        let Some(candidate) = round.component(completion, inputs, component)? else {
            continue;
        };
        let description = match candidate {
            PackageReviewComponent::Description(description) => description,
            PackageReviewComponent::Source(source) => {
                source.publish_component().map_err(|diagnostics| {
                    completion.compilation_failure(
                        compilation_products::CompileFailure::source_completion(
                            diagnostics,
                            source.source_completion_accounting().clone(),
                        ),
                    )
                })?
            }
        };
        descriptions.push(description);
    }
    Ok(descriptions)
}
