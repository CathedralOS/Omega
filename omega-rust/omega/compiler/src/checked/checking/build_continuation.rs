//! Admit and execute the build machine, then type its generated-source continuation.

use artifacts::compile_timings::CompileTimings;
use compilation_products::CheckedExecutionRequest;
use diagnostics::Diagnostic;
use package_compilation::BuildSourceCustody;
use package_compilation::PackageCompilationInputs;
use std::path::Path;
use std::sync::Arc;

/// One coherent final typed frontend and its evaluated build selection.
#[derive(Clone)]
pub(crate) struct BuiltCheckedProgram {
    pub(super) typed: typed_trees::TypedTrees,
    pub(super) selected_target_machine_declarations:
        provider_planning::target_machines::SelectedTargetMachineDeclarations,
    pub(super) pending_pre_checks: Vec<build_time_evaluation::PreCheckEvaluation>,
    pub(super) computed_build_config: Arc<build_evaluation::ComputedBuildConfig>,
    /// The normalized restricted build-host requests the admitted activation
    /// asked of the host before it executed.
    pub(super) restricted_build_requests:
        Vec<package_compilation::build_execution::RestrictedBuildRequest>,
    /// The validated `builder.application` declaration retained from source
    /// assembly. Its name supplies the publication `.app` basename and inner
    /// executable leaf; its artifact-only intent narrows the admitted route.
    pub(super) application: Option<build_declarations::ApplicationDeclaration>,
    pub(super) selected_build_machine_identity: Option<String>,
}

/// The complete deterministic product of one child's source assembly and
/// typed frontend: everything `evaluate_build_and_continue` consumes before
/// the declaration-authority check, retained so a later child of the same
/// checkpoint under identical frontier inputs reuses it exactly.
#[derive(Clone)]
pub(crate) struct TypedFrontier {
    frontend: CheckedFrontend,
    source_file_count: usize,
    generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    base_sources: Arc<source::SourceMap>,
    application: Option<build_declarations::ApplicationDeclaration>,
}

/// Consume a prepared typed program. Source preparation owns any reuse;
/// declaration authority and Build effects are admitted for this activation.
pub(crate) fn evaluate_build_and_continue(
    root_path: &Path,
    child: CheckedExecutionRequest<'_>,
    frontier: TypedFrontier,
    timings: &mut CompileTimings,
) -> Result<(BuiltCheckedProgram, BuildSourceCustody), Vec<Diagnostic>> {
    let CheckedExecutionRequest {
        selected_target_profile,
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
    // CLI aliases end at request admission. Every source, build, provider, and
    // artifact consumer below observes only the catalog's canonical spelling.
    let target_name = selected_target_profile.map(target::TargetProfile::target_name);
    let TypedFrontier {
        frontend,
        mut source_file_count,
        mut generated_source_custody,
        base_sources,
        application,
    } = frontier;
    let build_evaluation::EvaluatedBuild {
        configuration: computed_build_config,
        restricted_build_requests,
        base_source_consumption_commitment,
    } = build_evaluation::evaluate_build(
        build_evaluation::BuildEvaluationRequest {
            root_path,
            typed: frontend.typing.typed(),
            build_source_id: frontend.build_source_id,
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
    let CheckedFrontend {
        typing,
        mut selected_target_machine_declarations,
        mut pending_pre_checks,
        ..
    } = frontend;
    let typed = if computed_build_config.generated_sources.is_empty() {
        typing.into_typed()
    } else {
        let package_inputs = package_inputs.ok_or_else(|| {
            vec![Diagnostic::error(
                "generated-source final compilation requires package-aware source custody",
            )]
        })?;
        let package_root = package_inputs
            .package_root(package_inputs.root())
            .expect("validated package inputs retain their root package");
        let extension = package_compilation::source_imports::admit_generated_extension(
            &base_sources,
            package_root,
            Some(package_inputs.root()),
            &computed_build_config.generated_sources,
        )?;
        source_file_count = source_file_count
            .checked_add(extension.custody().len())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "generated package source count exceeds the compiler range",
                )]
            })?;
        generated_source_custody.extend(extension.custody().iter().cloned());
        let (typed, declarations, extension_pre_checks) = continue_generated_sources(
            typing,
            &base_sources,
            extension,
            selected_target_machine_declarations,
            target_name,
            package_inputs,
            timings,
        )?;
        selected_target_machine_declarations = declarations;
        pending_pre_checks.extend(extension_pre_checks);
        typed
    };
    let selected_build_machine_identity =
        computed_build_config.selected_machine_identity(&typed)?;
    Ok((
        BuiltCheckedProgram {
            typed,
            selected_target_machine_declarations,
            pending_pre_checks,
            computed_build_config: Arc::new(computed_build_config),
            restricted_build_requests,
            application,
            selected_build_machine_identity,
        },
        BuildSourceCustody {
            source_file_count,
            build_execution_profile,
            generated_source_custody,
            base_source_consumption_commitment,
        },
    ))
}

#[derive(Clone)]
struct CheckedFrontend {
    typing: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    selected_target_machine_declarations:
        provider_planning::target_machines::SelectedTargetMachineDeclarations,
    build_source_id: Option<source::SourceId>,
    pending_pre_checks: Vec<build_time_evaluation::PreCheckEvaluation>,
}

pub(super) fn lower_typed_frontier(
    source_file_count: usize,
    mut syntax: package_compilation::source_assembly::AssembledSyntax,
    target_name: Option<&str>,
    execution_profile_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<TypedFrontier, Vec<Diagnostic>> {
    let generated_source_custody = std::mem::take(&mut syntax.generated_source_custody);
    let base_sources = syntax.sources.clone();
    let application = syntax.application.take();
    // Build-scope sources select their target-scoped declarations against
    // the admitted execution profile; product sources against the target.
    // An unprofiled host resolves to its `NativeTarget` triple. Build-time
    // evaluation resolves and checks its own probe programs, so it must see
    // the selected program: an unselected per-target leaf (std's
    // `ConsoleNativeProvider::read_byte`) is inert and its callers fail.
    let build_scope_sources = std::mem::take(&mut syntax.build_scope_sources);
    let selected_target_machine_declarations =
        provider_planning::target_machines::filter_target_machines_by_scope(
            &mut syntax.syntax_trees,
            target_name,
            execution_profile_name,
            &build_scope_sources,
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
                    syntax: &syntax.syntax_trees,
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
    selected_target_machine_declarations.admit_provider_default_calls(typing_base.typed())?;
    let pending_pre_checks = pre_check
        .evaluate_or_defer(typing_base.typed_mut())?
        .into_iter()
        .collect();
    Ok(TypedFrontier {
        frontend: CheckedFrontend {
            typing: typing_base,
            selected_target_machine_declarations,
            build_source_id,
            pending_pre_checks,
        },
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
    target_name: Option<&str>,
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
        .filter_generated_extension(&mut extension_syntax, target_name)?;
    let seeded = timings.record(
        artifacts::compile_timings::SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
        || {
            syntax_trees_to_symbol_resolved_trees::resolve_extension(
                syntax_trees_to_symbol_resolved_trees::ExtensionRequest {
                    base: resolved_base,
                    syntax: &extension_syntax,
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
    selected_target_machine_declarations.admit_provider_default_calls(&typed)?;
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
