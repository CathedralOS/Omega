//! Admit and execute the build machine, then type its generated-source continuation.

use super::CheckedChildExecution;
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
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

/// Source custody spanning the pre-build admission and generated extension.
#[derive(Clone)]
pub(crate) struct BuildSourceCustody {
    pub(super) source_file_count: usize,
    pub(super) build_execution_profile: Option<target::TargetProfile>,
    pub(super) generated_source_custody:
        Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    pub(super) base_source_consumption_commitment:
        Option<package_compilation::PackageSourceConsumptionCommitment>,
}

/// One admitted child's route to its typed frontier.
pub(crate) enum CheckedFrontierInput {
    /// Source assembled fresh for this child; the typed frontend is lowered
    /// from it and retained for a later child carrying the same frontier key.
    Assembled {
        source_file_count: usize,
        syntax: build_evaluation::sources::AssembledSyntax,
    },
    /// A typed frontier retained from an earlier child of the same immutable
    /// source checkpoint with identical [`package_compilation::SourceAssemblyInputs`].
    /// Assembly and the typed frontend are deterministic in those inputs; the child's
    /// per-activation custody admission, declaration authority check, build
    /// execution and provider settlement all still run against its live
    /// request, and only work no later input could alter is skipped.
    Retained(TypedFrontier),
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

/// Typed frontiers one immutable source checkpoint produced for children
/// already compiled, keyed by the package owner's exact source assembly inputs.
/// `PreparedCheckedSource` clones share the map, so a later pass over the
/// same frontier reuses the typed program instead of re-assembling and
/// re-lowering identical source.
pub(crate) type TypedFrontierMemo =
    std::sync::Mutex<Vec<(package_compilation::SourceAssemblyInputs, TypedFrontier)>>;

/// One checkpoint-shared memo and the key a fresh frontier is stored under.
pub(crate) struct TypedFrontierMemoSlot {
    pub(crate) memo: std::sync::Arc<TypedFrontierMemo>,
    pub(crate) key: package_compilation::SourceAssemblyInputs,
}

/// Assembled-syntax entry for callers that never retain a typed frontier
/// (the multi-target shared-build route, which keys its own reuse on the
/// whole build input instead).
pub(crate) fn evaluate_build_and_continue(
    root_path: &Path,
    child: CheckedChildExecution<'_>,
    source_file_count: usize,
    syntax: build_evaluation::sources::AssembledSyntax,
    timings: &mut CompileTimings,
) -> Result<(BuiltCheckedProgram, BuildSourceCustody), Vec<Diagnostic>> {
    evaluate_frontier_and_continue(
        root_path,
        child,
        CheckedFrontierInput::Assembled {
            source_file_count,
            syntax,
        },
        None,
        timings,
    )
}

pub(crate) fn evaluate_frontier_and_continue(
    root_path: &Path,
    child: CheckedChildExecution<'_>,
    frontier: CheckedFrontierInput,
    memo_slot: Option<TypedFrontierMemoSlot>,
    timings: &mut CompileTimings,
) -> Result<(BuiltCheckedProgram, BuildSourceCustody), Vec<Diagnostic>> {
    let CheckedChildExecution {
        selected_target_profile,
        build_execution_profile,
        package_inputs,
        build_dir,
        filesystem_sponsor,
        evaluation_sponsor,
        build_snapshot,
        optimization_rollback: _,
        // Build evaluation is this continuation's last stop before provider
        // settlement; the caller extracts the discovery write target first.
        independent_component_discovery: _,
        mut restricted_build_grants,
        permit_unsettled_fused_service_fields: _,
    } = child;
    // CLI aliases end at request admission. Every source, build, provider, and
    // artifact consumer below observes only the catalog's canonical spelling.
    let target_name = selected_target_profile.map(target::TargetProfile::target_name);
    // `None` admits an unprofiled compiler host; the build-scope filter then
    // selects against the host's `NativeTarget` triple.
    let execution_profile_name = build_execution_profile.map(target::TargetProfile::target_name);
    let (
        mut source_file_count,
        mut generated_source_custody,
        base_sources,
        application,
        mut frontend,
    ) = match frontier {
        CheckedFrontierInput::Assembled {
            source_file_count,
            syntax,
        } => {
            let generated_source_custody = syntax.generated_source_custody.clone();
            let base_sources = syntax.sources.clone();
            let application = syntax.application.clone();
            let frontend = lower_checked_frontend(
                syntax,
                target_name,
                execution_profile_name,
                package_inputs,
                timings,
            )?;
            if let Some(slot) = memo_slot {
                let mut entries = slot
                    .memo
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                entries.retain(|(key, _)| *key != slot.key);
                entries.push((
                    slot.key,
                    TypedFrontier {
                        frontend: frontend.clone(),
                        source_file_count,
                        generated_source_custody: generated_source_custody.clone(),
                        base_sources: base_sources.clone(),
                        application: application.clone(),
                    },
                ));
            }
            (
                source_file_count,
                generated_source_custody,
                base_sources,
                application,
                frontend,
            )
        }
        CheckedFrontierInput::Retained(frontier) => {
            let TypedFrontier {
                frontend,
                source_file_count,
                generated_source_custody,
                base_sources,
                application,
            } = frontier;
            (
                source_file_count,
                generated_source_custody,
                base_sources,
                application,
                frontend,
            )
        }
    };
    let package_authority_verdict = if let Some(package_inputs) = package_inputs {
        Some(crate::checked::package::declaration_admission::validate_authored_declaration_selections_before_build(
            frontend.typed(),
            package_inputs,
            &generated_source_custody,
            timings,
        )?)
    } else {
        None
    };
    let build_machine_filesystem_scope = build_evaluation::prepare_filesystem_scope(
        root_path,
        package_inputs,
        &base_sources,
        build_execution_profile,
        build_dir,
        filesystem_sponsor,
        build_snapshot.as_ref(),
    )?;
    let admitted_build = build_evaluation::admit_build_program(
        frontend.typed(),
        frontend.build_source_id,
        &build_machine_filesystem_scope,
        evaluation_sponsor.as_ref(),
        application
            .as_ref()
            .is_some_and(|application| application.artifact_only),
    )?;
    // Consent joins the admitted request meanings before any build effect.
    // Keep the selected machine bound across execution, not in a second
    // checkpoint that only transports this continuation's local variables.
    let admitted_build_symbol = admitted_build.selected_build_machine_symbol();
    let restricted_build_requests = admitted_build.restricted_build_requests();
    if let Some(grants) = restricted_build_grants.as_mut() {
        for request in &restricted_build_requests {
            grants.admit(request)?;
        }
    }
    let computed_build_config = admitted_build.execute()?;
    if computed_build_config.selected_build_machine_symbol != admitted_build_symbol {
        return Err(vec![Diagnostic::error(
            "build execution returned a selected symbol different from its admitted checkpoint",
        )]);
    }
    // The activation's grant guard ends with execution, before source continuation.
    drop(restricted_build_grants);
    let selected_build_machine_symbol = computed_build_config.selected_build_machine_symbol;
    if !computed_build_config.generated_sources.is_empty() {
        let package_inputs = package_inputs.ok_or_else(|| {
            vec![Diagnostic::error(
                "generated-source final compilation requires package-aware source custody",
            )]
        })?;
        let package_root = package_inputs
            .package_root(package_inputs.root())
            .expect("validated package inputs retain their root package");
        let extension = build_evaluation::sources::retain_generated_syntax_extension(
            &base_sources,
            package_root,
            Some(package_inputs.root()),
            &computed_build_config.generated_sources,
        )?;
        source_file_count = source_file_count
            .checked_add(extension.source_count())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "generated package source count exceeds the compiler range",
                )]
            })?;
        generated_source_custody.extend(extension.generated_source_custody().iter().cloned());
        let CheckedFrontend {
            typing,
            selected_target_machine_declarations,
            build_source_id,
            mut pending_pre_checks,
        } = frontend;
        let CheckedFrontendTyping::Continuable(typing_base) = typing else {
            return Err(vec![Diagnostic::error(
                "generated-source continuation lost its retained frontend base",
            )]);
        };
        let (typed, selected_target_machine_declarations, extension_pre_checks) =
            try_seeded_extension(
                typing_base,
                &base_sources,
                extension,
                selected_target_machine_declarations,
                target_name,
                Some(package_inputs),
                timings,
            )?;
        pending_pre_checks.extend(extension_pre_checks);
        frontend = CheckedFrontend {
            typing: CheckedFrontendTyping::Complete(typed),
            selected_target_machine_declarations,
            build_source_id,
            pending_pre_checks,
        };
    }
    let CheckedFrontend {
        typing,
        selected_target_machine_declarations,
        pending_pre_checks,
        ..
    } = frontend;
    let typed = typing.into_typed();
    let selected_build_machine_identity = selected_build_machine_symbol
        .map(|selected| {
            let machine = typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == selected)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected build machine disappeared before final checking",
                    )]
                })?;
            typed
                .normalized_machine_overload_identity(machine)
                .map(|identity| identity.identity().to_owned())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected build machine has no canonical callable identity",
                    )]
                })
        })
        .transpose()?;
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
            base_source_consumption_commitment: package_authority_verdict
                .as_ref()
                .map(|verdict| verdict.base_source_consumption_commitment()),
        },
    ))
}

#[derive(Clone)]
struct CheckedFrontend {
    typing: CheckedFrontendTyping,
    selected_target_machine_declarations:
        provider_planning::target_machines::SelectedTargetMachineDeclarations,
    build_source_id: Option<source::SourceId>,
    pending_pre_checks: Vec<build_time_evaluation::PreCheckEvaluation>,
}

#[derive(Clone)]
enum CheckedFrontendTyping {
    Continuable(symbol_resolved_trees_to_typed_trees::SeededTypingBase),
    Complete(typed_trees::TypedTrees),
}

impl CheckedFrontendTyping {
    fn typed(&self) -> &typed_trees::TypedTrees {
        match self {
            Self::Continuable(base) => base.typed(),
            Self::Complete(typed) => typed,
        }
    }

    fn into_typed(self) -> typed_trees::TypedTrees {
        match self {
            Self::Continuable(base) => base.into_typed(),
            Self::Complete(typed) => typed,
        }
    }
}

impl CheckedFrontend {
    fn typed(&self) -> &typed_trees::TypedTrees {
        self.typing.typed()
    }
}

fn lower_checked_frontend(
    mut syntax: build_evaluation::sources::AssembledSyntax,
    target_name: Option<&str>,
    execution_profile_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<CheckedFrontend, Vec<Diagnostic>> {
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
    // Build evaluation consumes this coherent private typed stage before the
    // final checked-tree lowering. Bind trait-valued parameter-field calls now
    // so the evaluator receives the same exact requirement identity that the
    // checker will subsequently validate and retain.
    validation::resolve_dynamic_call_targets(typing_base.typed_mut())?;
    Ok(CheckedFrontend {
        typing: CheckedFrontendTyping::Continuable(typing_base),
        selected_target_machine_declarations,
        build_source_id,
        pending_pre_checks,
    })
}

fn try_seeded_extension(
    base: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    base_sources: &Arc<source::SourceMap>,
    extension: build_evaluation::sources::RetainedGeneratedSyntaxExtension,
    selected_target_machine_declarations:
        provider_planning::target_machines::SelectedTargetMachineDeclarations,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<
    (
        typed_trees::TypedTrees,
        provider_planning::target_machines::SelectedTargetMachineDeclarations,
        Vec<build_time_evaluation::PreCheckEvaluation>,
    ),
    Vec<Diagnostic>,
> {
    let retained_prefix = base.typed().clone();
    let mut wire_schema_frontier = retained_prefix.wire_schemas().len();
    let (extension_units, sources) = extension.into_pre_resolution_inputs(base_sources)?;
    let mut extension_syntax = syntax_trees::SyntaxTrees::new(source::SourceId(base_sources.len()));
    let mut pre_checks = Vec::with_capacity(extension_units.len());
    // Normalize each unit against the same immutable predecessor. Retained
    // nominal arguments are selectable; sibling units and base templates are
    // not silently re-normalized together.
    let resolved_base = base.resolved_base_for_extension();
    let authority = package_inputs
        .filter(|_| !extension_units.is_empty())
        .map(|inputs| {
            Arc::new(inputs.clone()) as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        });
    for unit in extension_units {
        let evaluated = build_time_evaluation::evaluate_pre_resolution(
            build_time_evaluation::BuildTimeEvaluationRequest {
                syntax_trees: unit,
                source_context: Some(build_time_evaluation::BuildTimeSourceContext {
                    sources: sources.clone(),
                    source_scoped_top_level_bindings: &[],
                    selection_authority: authority.clone(),
                    retained_base: Some(&resolved_base),
                }),
            },
        )?;
        let (unit, pre_check) = evaluated.into_syntax_and_pre_check();
        extension_syntax.extend_from(&unit);
        pre_checks.push(pre_check);
    }
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
    let rebased = seeded
        .rebase_authored_selections_for_typed_continuation(
            base.typed().authored_declaration_selections(),
        )
        .map_err(|(_, error)| {
            vec![Diagnostic::error(format!(
                "generated-source authored-selection suffix could not join the retained typed base: {error:?}"
            ))]
        })?;
    let mut typed = match timings.record_result(
        artifacts::compile_timings::SYMBOL_RESOLVED_TREES_TO_TYPED_TREES,
        || symbol_resolved_trees_to_typed_trees::lower_seeded_extension(rebased, base),
    ) {
        Ok(typed) => Ok(typed),
        Err((
            _,
            symbol_resolved_trees_to_typed_trees::SeededContinuationError::Lowering(
                diagnostic,
            ),
        )) => Err(vec![diagnostic]),
        Err((
            _,
            symbol_resolved_trees_to_typed_trees::SeededContinuationError::UnsupportedExtensionShape,
        )) => Err(vec![Diagnostic::error(
            "generated source uses a declaration shape not yet supported by retained-checkpoint continuation; reconstructing a second frontend is forbidden",
        )]),
        Err((_, error)) => Err(vec![Diagnostic::error(format!(
            "generated-source continuation violated its retained-base invariant: {error:?}"
        ))]),
    }?;
    selected_target_machine_declarations.admit_provider_default_calls(&typed)?;
    let mut pending_pre_checks = Vec::new();
    for pre_check in pre_checks {
        if let Some(pending) =
            pre_check.evaluate_extension_or_defer(&mut typed, wire_schema_frontier)?
        {
            pending_pre_checks.push(pending);
        }
        wire_schema_frontier = typed.wire_schemas().len();
    }
    if !symbol_resolved_trees_to_typed_trees::retained_typed_base_is_exact_prefix(
        &retained_prefix,
        &typed,
    ) {
        return Err(vec![Diagnostic::error(
            "generated-source pre-check evaluation changed the retained typed base",
        )]);
    }
    Ok((
        typed,
        selected_target_machine_declarations,
        pending_pre_checks,
    ))
}
