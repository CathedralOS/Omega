//! Admit and execute Build while retaining pending Product implementations.
//!
//! Dependency preparation needs Build's result and declaration descriptions,
//! not an executable whole-package Product. An actual Product request completes
//! its loaded module bodies, then types generated sources without replaying Build.

use artifacts::compile_timings::CompileTimings;
use compilation_products::{
    BuildAdmission, BuiltTypedProgram, CheckedExecutionRequest, CompileFailure,
    TypedCompilationFrontier,
};
use diagnostics::Diagnostic;
use package_compilation::BuildSourceCustody;
use package_compilation::PackageCompilationInputs;
use std::path::Path;
use std::sync::Arc;

/// Consume a prepared typed program. Source preparation owns any reuse;
/// declaration authority and Build effects are admitted for this activation.
pub(crate) fn evaluate_build_and_continue(
    root_path: &Path,
    child: CheckedExecutionRequest<'_>,
    frontier: TypedCompilationFrontier,
    timings: &mut CompileTimings,
    pure_build_compilation: Option<&build_evaluation::PureBuildCompilation>,
) -> Result<(BuiltTypedProgram, BuildSourceCustody), CompileFailure> {
    let CheckedExecutionRequest {
        selected_target_profile: _,
        build_execution_profile,
        package_inputs,
        build_dir,
        filesystem_sponsor,
        evaluation_sponsor,
        build_snapshot,
        optimization_rollback: _,
        restricted_build_grants,
        permit_unsettled_fused_service_fields: _,
        settled_check_reuse,
    } = child;
    let TypedCompilationFrontier {
        mut typing,
        pending_bodies,
        build_source_id,
        mut pending_pre_checks,
        source_file_count,
        generated_source_custody,
        base_sources,
        application,
    } = frontier;
    let build_evaluation::EvaluatedBuild {
        configuration: computed_build_config,
        restricted_build_requests,
        base_source_consumption_commitment,
        retained_check: retained_build_check,
    } = build_evaluation::evaluate_build(
        build_evaluation::BuildEvaluationRequest {
            pure_build_compilation,
            root_path,
            typed: typing.typed_mut(),
            pre_checks: &mut pending_pre_checks,
            build_source_id,
            sources: &base_sources,
            package_inputs,
            generated_source_custody: &generated_source_custody,
            execution_profile: build_execution_profile,
            build_directory: build_dir,
            filesystem_sponsor,
            evaluation_sponsor,
            snapshot: build_snapshot.as_ref(),
            restricted_build_grants,
            artifact_only: application
                .as_ref()
                .is_some_and(|application| application.artifact_only),
            prior_check: settled_check_reuse.prior.build.as_ref(),
            retain_check: settled_check_reuse.retain,
        },
        timings,
    )?;
    let sources = BuildSourceCustody {
        source_file_count,
        build_execution_profile,
        generated_source_custody,
        base_source_consumption_commitment,
    };
    let admission = BuildAdmission {
        application,
        provider_defaults: Vec::new(),
        selected_build_machine_identity: computed_build_config
            .selected_machine_identity(typing.typed())?,
        restricted_build_requests,
    };
    Ok((
        BuiltTypedProgram {
            typing,
            pending_bodies,
            base_sources,
            pending_pre_checks,
            computed_build_config,
            admission,
            retained_build_check,
        },
        sources,
    ))
}

pub(super) fn lower_typed_frontier(
    source_file_count: usize,
    mut syntax: package_compilation::source_assembly::AssembledSyntax,
    execution_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<TypedCompilationFrontier, Vec<Diagnostic>> {
    let generated_source_custody = std::mem::take(&mut syntax.generated_source_custody);
    let base_sources = syntax.sources.clone();
    let application = syntax.application.take();
    // Only build helpers select an execution target here. Product families
    // retain a canonical frontend shared by every requested product target.
    let build_scope_sources = std::mem::take(&mut syntax.build_scope_sources);
    let execution = execution_profile
        .or_else(target::TargetProfile::host_if_supported)
        .ok_or_else(|| vec![Diagnostic::error(
            "no target profile named and this host has no catalogued Omega deployment profile; name an exact target profile explicitly",
        )])?;
    // Build execution still requires an implemented host shape, but that
    // projection is not the identity used to choose source declarations.
    if execution.native_realization().is_none() {
        return Err(vec![Diagnostic::error(format!(
            "native realization for target profile `{}` is not implemented",
            execution.target_name(),
        ))]);
    }
    build_evaluation::select_build_helpers(
        &mut syntax.syntax_trees,
        execution,
        &build_scope_sources,
    );
    let evaluated = build_time_evaluation::evaluate_pre_resolution(
        build_time_evaluation::BuildTimeEvaluationRequest {
            syntax_trees: syntax.syntax_trees,
            source_context: Some(build_time_evaluation::BuildTimeSourceContext {
                sources: syntax.sources.clone(),
                source_scoped_top_level_bindings: &syntax.source_scoped_top_level_bindings,
                selection_authority: package_inputs.map(|inputs| {
                    Arc::new(inputs.clone())
                        as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
                }),
                retained_base: None,
            }),
        },
    )?;
    let (syntax_trees, pre_check) = evaluated.into_syntax_and_pre_check();
    syntax.syntax_trees = syntax_trees;
    let build_source_id = syntax.build_source_id;
    let mut pending_bodies = timings.record(
        artifacts::compile_timings::SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
        || {
            syntax_trees_to_symbol_resolved_trees::resolve_deferred(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                    syntax: syntax.syntax_trees,
                    sources: Some(syntax.sources),
                    top_level_bindings: syntax.source_scoped_top_level_bindings,
                },
            )
        },
    )?;
    // Build execution may use its entire admitted declaration context. Product
    // descriptions keep real pending implementations, not callable empty bodies.
    loop {
        let build_bodies = pending_bodies
            .pending_machines()
            .filter(|symbol| {
                pending_bodies
                    .trees()
                    .symbols
                    .symbol_provenance_source_span(*symbol)
                    .and_then(|span| pending_bodies.trees().symbols.source_file(span))
                    .is_some_and(|file| {
                        file.origin == source::SourceOrigin::Toolchain
                            || file.dependency_scope == source::DependencyScope::Build
                    })
            })
            .collect::<Vec<_>>();
        if build_bodies.is_empty() {
            break;
        }
        pending_bodies = timings
            .record(
                artifacts::compile_timings::SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
                || pending_bodies.complete_machine_bodies(&build_bodies),
            )?
            .into_resolved_frontier();
        // Body-driven type completion can append attached implementations.
        // Reapply the same scope selection to those owners, never to Product.
    }
    let typing_base = timings.record(
        artifacts::compile_timings::SYMBOL_RESOLVED_TREES_TO_TYPED_TREES,
        || {
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
                pending_bodies.shared_trees(),
            )
            .map_err(|diagnostic| vec![diagnostic])
        },
    )?;
    // Build selects its work from this declaration frontier and settles that
    // work in the retained arenas. Product implementations are still pending;
    // evaluating the whole continuation here would demand unrelated layouts
    // and constants. Successful Build checking leaves only Product work.
    let pending_pre_checks = vec![pre_check];
    Ok(TypedCompilationFrontier {
        typing: typing_base,
        pending_bodies,
        build_source_id,
        pending_pre_checks,
        source_file_count,
        generated_source_custody,
        base_sources,
        application,
    })
}

/// An actual Product demand consumes its retained Build exactly once and
/// completes the implementation sources already selected by its module closure.
/// Generated source is a later stratum, never a reason to rerun authored Build.
pub(super) fn complete_product_frontend(
    built: BuiltTypedProgram,
    sources: &mut BuildSourceCustody,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<
    (
        typed_trees::TypedTrees,
        Vec<build_time_evaluation::PreCheckEvaluation>,
        Arc<compilation_products::ComputedBuildConfig>,
        BuildAdmission,
    ),
    Vec<Diagnostic>,
> {
    artifacts::compile_timings::counters::record_product_source_completion();
    let BuiltTypedProgram {
        mut typing,
        pending_bodies,
        base_sources,
        pending_pre_checks,
        computed_build_config,
        admission,
        retained_build_check: _,
    } = built;
    // Build evaluation may have realized concrete data declarations. Admit
    // their exact generated-symbol suffix before the resolver allocates any
    // Product body locals, retaining one shared predecessor for both owners.
    let mut pending_bodies = pending_bodies;
    loop {
        pending_bodies = typing.join_generated_symbols(pending_bodies)?;
        let selected = pending_bodies.pending_machines().collect::<Vec<_>>();
        if selected.is_empty() {
            break;
        }
        let resolved = timings.record(
            artifacts::compile_timings::SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
            || pending_bodies.complete_machine_bodies(&selected),
        )?;
        (typing, pending_bodies) = timings.record(
            artifacts::compile_timings::SYMBOL_RESOLVED_TREES_TO_TYPED_TREES,
            || typing.complete_bodies(resolved),
        )?;
        // Keep the returned resolver owner: newly synthesized declarations may
        // carry further bodies, and their typing must use this exact successor.
    }
    // Complete authored evaluation before admitting generated source. This is
    // the first point where Product bodies exist, and the resulting typed base
    // is the immutable predecessor of every generated cohort.
    let mut deferred_pre_checks = Vec::new();
    for pre_check in pending_pre_checks {
        if let Some(pending) = pre_check.evaluate_or_defer(typing.typed_mut())? {
            deferred_pre_checks.push(pending);
        }
    }
    let mut pending_pre_checks = deferred_pre_checks;
    let typed = if let Some((extension, package_inputs)) = sources.admit_generated_extension(
        &base_sources,
        package_inputs,
        &computed_build_config.generated_sources,
    )? {
        let (typed, checks) =
            continue_generated_sources(typing, &base_sources, extension, package_inputs, timings)?;
        pending_pre_checks.extend(checks);
        typed
    } else {
        typing.into_typed()
    };
    Ok((typed, pending_pre_checks, computed_build_config, admission))
}

fn continue_generated_sources(
    mut base: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    base_sources: &Arc<source::SourceMap>,
    extension: package_compilation::source_imports::GeneratedSourceExtension,
    package_inputs: &PackageCompilationInputs,
    timings: &mut CompileTimings,
) -> Result<
    (
        typed_trees::TypedTrees,
        Vec<build_time_evaluation::PreCheckEvaluation>,
    ),
    Vec<Diagnostic>,
> {
    let (sources, units) = extension.parse(base_sources, timings)?;
    let wire_schema_frontier = base.typed().wire_schemas().len();
    let resolved_base = base.resolved_base_for_extension()?;
    let (extension_syntax, pre_checks) =
        build_time_evaluation::PreResolutionEvaluation::evaluate_extension(
            units,
            source::SourceId(base_sources.len()),
            build_time_evaluation::BuildTimeSourceContext {
                sources: sources.clone(),
                source_scoped_top_level_bindings: &[],
                selection_authority: Some(Arc::new(package_inputs.clone())),
                retained_base: Some(&resolved_base),
            },
        )?;
    let seeded = timings.record(
        artifacts::compile_timings::SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
        || {
            syntax_trees_to_symbol_resolved_trees::resolve_extension(
                syntax_trees_to_symbol_resolved_trees::ExtensionRequest {
                    base: resolved_base,
                    syntax: extension_syntax,
                    sources,
                    top_level_bindings: Vec::new(),
                },
            )
        },
    )?;
    let mut typed = timings.record(
        artifacts::compile_timings::SYMBOL_RESOLVED_TREES_TO_TYPED_TREES,
        || base.lower_extension(seeded),
    )?;
    let pending_pre_checks =
        build_time_evaluation::PreCheckEvaluation::evaluate_extension_or_defer(
            pre_checks,
            wire_schema_frontier,
            &mut typed,
        )?;
    Ok((typed, pending_pre_checks))
}
