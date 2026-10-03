//! Admit and execute the build machine, then type its generated-source continuation.

use artifacts::compile_timings::CompileTimings;
use compilation_products::{
    BuildAdmission, BuiltTypedProgram, CheckedExecutionRequest, TypedCompilationFrontier,
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
) -> Result<
    (
        BuiltTypedProgram,
        BuildSourceCustody,
        compilation_products::BuildEvaluationAccounting,
    ),
    Vec<Diagnostic>,
> {
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
    } = child;
    let TypedCompilationFrontier {
        typing,
        mut selected_target_machine_declarations,
        build_source_id,
        mut pending_pre_checks,
        source_file_count,
        generated_source_custody,
        base_sources,
        application,
    } = frontier;
    let build_evaluation::EvaluatedBuild {
        configuration: computed_build_config,
        accounting,
        restricted_build_requests,
        base_source_consumption_commitment,
    } = build_evaluation::evaluate_build(
        build_evaluation::BuildEvaluationRequest {
            pure_build_compilation,
            root_path,
            typed: typing.typed(),
            target_machine_declarations: &selected_target_machine_declarations,
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
        },
        timings,
    )?;
    let mut sources = BuildSourceCustody {
        source_file_count,
        build_execution_profile,
        generated_source_custody,
        base_source_consumption_commitment,
    };
    let typed = if let Some((extension, package_inputs)) = sources.admit_generated_extension(
        &base_sources,
        package_inputs,
        &computed_build_config.generated_sources,
    )? {
        let (typed, declarations, extension_pre_checks) = continue_generated_sources(
            typing,
            &base_sources,
            extension,
            selected_target_machine_declarations,
            package_inputs,
            timings,
        )?;
        selected_target_machine_declarations = declarations;
        pending_pre_checks.extend(extension_pre_checks);
        typed
    } else {
        typing.into_typed()
    };
    let admission = BuildAdmission {
        application,
        selected_build_machine_identity: computed_build_config.selected_machine_identity(&typed)?,
        restricted_build_requests,
    };
    Ok((
        BuiltTypedProgram {
            typed,
            selected_target_machine_declarations,
            pending_pre_checks,
            computed_build_config,
            admission,
        },
        sources,
        accounting,
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
    let selected_target_machine_declarations =
        provider_planning::target_machines::prepare_target_machines(
            &mut syntax.syntax_trees,
            Some(&syntax.sources),
            Some(provider_planning::target_machines::BuildTargetScope {
                execution,
                sources: &build_scope_sources,
            }),
        )?;
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
    selected_target_machine_declarations
        .release_provider_default_declarations(&mut syntax.syntax_trees);
    let build_source_id = syntax.build_source_id;
    let resolved = timings.record(
        artifacts::compile_timings::SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
        || {
            syntax_trees_to_symbol_resolved_trees::resolve(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                    syntax: syntax.syntax_trees,
                    sources: Some(syntax.sources),
                    top_level_bindings: syntax.source_scoped_top_level_bindings,
                },
            )
        },
    )?;
    let mut typing_base = timings.record(
        artifacts::compile_timings::SYMBOL_RESOLVED_TREES_TO_TYPED_TREES,
        || {
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(
                resolved,
            )
            .map_err(|diagnostic| vec![diagnostic])
        },
    )?;
    for scope in [
        source::DependencyScope::Build,
        source::DependencyScope::Product,
    ] {
        selected_target_machine_declarations.admit_provider_default_calls(
            provider_planning::ProviderPlanningContext::new(typing_base.typed(), scope),
        )?;
    }
    let pending_pre_checks = pre_check
        .evaluate_or_defer(typing_base.typed_mut())?
        .into_iter()
        .collect();
    Ok(TypedCompilationFrontier {
        typing: typing_base,
        selected_target_machine_declarations,
        build_source_id,
        pending_pre_checks,
        source_file_count,
        generated_source_custody,
        base_sources,
        application,
    })
}

fn continue_generated_sources(
    base: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    base_sources: &Arc<source::SourceMap>,
    extension: package_compilation::source_imports::GeneratedSourceExtension,
    selected_target_machine_declarations:
        provider_planning::target_machines::SelectedTargetMachineDeclarations,
    package_inputs: &PackageCompilationInputs,
    timings: &mut CompileTimings,
) -> Result<
    (
        typed_trees::TypedTrees,
        provider_planning::target_machines::SelectedTargetMachineDeclarations,
        Vec<build_time_evaluation::PreCheckEvaluation>,
    ),
    Vec<Diagnostic>,
> {
    let (sources, units) = extension.parse(base_sources, timings)?;
    let retained_prefix = base.typed().clone();
    let resolved_base = base.resolved_base_for_extension();
    let (mut extension_syntax, pre_checks) =
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
    let selected_target_machine_declarations = selected_target_machine_declarations
        .filter_generated_extension(&mut extension_syntax, Some(&sources))?;
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
    for scope in [
        source::DependencyScope::Build,
        source::DependencyScope::Product,
    ] {
        selected_target_machine_declarations.admit_provider_default_calls(
            provider_planning::ProviderPlanningContext::new(&typed, scope),
        )?;
    }
    let pending_pre_checks =
        build_time_evaluation::PreCheckEvaluation::evaluate_extension_or_defer(
            pre_checks,
            &retained_prefix,
            &mut typed,
        )?;
    Ok((
        typed,
        selected_target_machine_declarations,
        pending_pre_checks,
    ))
}
