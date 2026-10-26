//! Execute a package-admitted review pass dependency-first. Policy chooses the
//! passes and admits exact reuse; Build owns compiler calls and continuations.

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

/// A demanded cache record may lack an executable component description.
/// Refresh that dependency while retaining completed reviews and the waiting
/// consumer's Build. Each restart adds a newly demanded occurrence; exact
/// input comparisons decide what work still applies after that refresh.
pub(super) fn execute_pass(
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
                recovered.push(round.into_retained());
                match error {
                    CompileResolvedPackageReviewsError::ComponentDescriptionNotRetained {
                        component,
                        purpose,
                        ..
                    } if fresh_components.insert((component, purpose)) => {}
                    error => return Err(error),
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
                    compiler::PackageProductOptions {
                        permit_unsettled_fused_service_fields: request
                            .permit_unsettled_fused_service_fields,
                        ..Default::default()
                    },
                )
                .map_err(|failure| completion.compilation_failure(failure))?
        }
    };

    let descriptions = component_descriptions(&build, &completion, round);
    let descriptions = match descriptions {
        Ok(descriptions) => descriptions,
        Err(error) => {
            if matches!(
                error,
                CompileResolvedPackageReviewsError::ComponentDescriptionNotRetained { .. }
            ) {
                pending_builds.push(PendingReviewBuild {
                    build,
                    entry,
                    snapshot,
                });
            }
            return Err(error);
        }
    };
    let build = build
        .with_independent_component_descriptions(descriptions)
        .map_err(|errors| completion.input_failure(errors))?;
    let inputs = build.package_inputs().clone();
    let product = compiler
        .review_package_build(build, maximum_policy_canonical_bytes)
        .map_err(|failure| completion.review_failure(failure))?;
    if let Some(cache_write) = round.retain(completion, product, inputs)? {
        // Policy has accepted the review and admitted a usable cache write.
        // Optional publication failure stays a miss; an actual consumer will
        // diagnose a failed demand through component_descriptions.
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
            PackageReviewComponent::Source(source) => source
                .publish_component()
                .map_err(|diagnostics| completion.compilation_failure(diagnostics.into()))?,
        };
        descriptions.push(description);
    }
    Ok(descriptions)
}
