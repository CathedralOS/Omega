//! Admit and execute the build machine, then type its generated-source continuation.

use super::CheckedChildExecution;
use crate::checking::phase_transitions::{
    resolve_seeded_syntax_extension, symbol_resolved_trees_to_seeded_base,
    syntax_trees_to_symbol_resolved_trees, type_seeded_extension,
};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::path::Path;
use std::sync::Arc;

/// One coherent final typed frontend and its evaluated build selection.
pub(super) struct BuiltCheckedProgram {
    pub(super) typed: typed_trees::TypedTrees,
    pub(super) selected_target_machine_declarations:
        build_evaluation::target_machines::SelectedTargetMachineDeclarations,
    pub(super) pending_pre_checks: Vec<build_time_evaluation::PreCheckEvaluation>,
    pub(super) computed_build_config: build_evaluation::ComputedBuildConfig,
    /// The validated `builder.application` declaration retained from source
    /// assembly. Its name supplies the publication `.app` basename and inner
    /// executable leaf; its artifact-only intent narrows the admitted route.
    pub(super) application: Option<build_declarations::ApplicationDeclaration>,
    pub(super) selected_build_machine_symbol: Option<symbols::SymbolHandle>,
    pub(super) selected_build_machine_identity: Option<String>,
}

/// Source custody spanning the pre-build admission and generated extension.
pub(super) struct BuildSourceCustody {
    pub(super) source_file_count: usize,
    pub(super) generated_source_custody:
        Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    pub(super) own_generated_sources: Vec<build_output::PackageGeneratedSource>,
    pub(super) base_source_consumption_commitment:
        Option<package_compilation::PackageSourceConsumptionCommitment>,
}

pub(super) fn evaluate_build_and_continue(
    root_path: &Path,
    child: CheckedChildExecution<'_>,
    mut source_file_count: usize,
    syntax: source_files_to_assembled_syntax::AssembledSyntax,
    timings: &mut CompileTimings,
) -> Result<(BuiltCheckedProgram, BuildSourceCustody), Vec<Diagnostic>> {
    let CheckedChildExecution {
        selected_target_profile,
        build_execution_profile,
        package_inputs,
        build_dir,
        filesystem_sponsor,
        evaluation_sponsor,
        replay_record,
        build_snapshot,
        optimization_rollback: _,
    } = child;
    // CLI aliases end at request admission. Every source, build, provider, and
    // artifact consumer below observes only the catalog's canonical spelling.
    let target_name = selected_target_profile.map(target::TargetProfile::target_name);
    // `None` admits an unprofiled compiler host; the build-scope filter then
    // selects against the host's `NativeTarget` triple.
    let execution_profile_name = build_execution_profile.map(target::TargetProfile::target_name);
    let mut generated_source_custody = syntax.generated_source_custody.clone();
    let base_sources = syntax.sources.clone();
    let application = syntax.application.clone();
    let mut frontend = lower_checked_frontend(
        syntax,
        target_name,
        execution_profile_name,
        package_inputs,
        timings,
    )?;
    let package_authority_verdict = if let Some(package_inputs) = package_inputs {
        Some(crate::package::declaration_admission::validate_authored_declaration_selections_before_build(
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
        replay_record,
        build_snapshot,
    )?;
    let admitted_build = build_evaluation::admit_build_program(
        frontend.typed(),
        frontend.build_source_id,
        &build_machine_filesystem_scope,
        evaluation_sponsor.as_ref(),
        selected_target_profile,
        application
            .as_ref()
            .is_some_and(|application| application.artifact_only),
    )?;
    let ExecutedBuildCheckpoint {
        frontend: executed_frontend,
        computed_build_config,
        package_authority_verdict,
        base_sources,
    } = (AdmittedBuildCheckpoint {
        frontend,
        admitted_build,
        package_authority_verdict,
        base_sources,
    })
    .execute()?;
    frontend = executed_frontend;
    let own_generated_sources = computed_build_config.generated_sources.clone();
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
        let extension = source_files_to_assembled_syntax::retain_generated_syntax_extension(
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
            computed_build_config,
            application,
            selected_build_machine_symbol,
            selected_build_machine_identity,
        },
        BuildSourceCustody {
            source_file_count,
            generated_source_custody,
            own_generated_sources,
            base_source_consumption_commitment: package_authority_verdict
                .as_ref()
                .map(|verdict| verdict.base_source_consumption_commitment()),
        },
    ))
}

struct CheckedFrontend {
    typing: CheckedFrontendTyping,
    selected_target_machine_declarations:
        build_evaluation::target_machines::SelectedTargetMachineDeclarations,
    build_source_id: Option<source::SourceId>,
    pending_pre_checks: Vec<build_time_evaluation::PreCheckEvaluation>,
}

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

/// One activation-local D18 checkpoint admitted before any authored build code
/// executes.
///
/// The coherent base frontend, exact prepared build projection, reach and
/// authority verdicts, package declaration verdict, and exact base source map
/// needed to bind a generated extension stay coupled across execution.
struct AdmittedBuildCheckpoint {
    frontend: CheckedFrontend,
    admitted_build: build_evaluation::AdmittedBuildProgram,
    package_authority_verdict:
        Option<crate::package::declaration_admission::AuthoredDeclarationAuthorityVerdict>,
    base_sources: Arc<source::SourceMap>,
}

struct ExecutedBuildCheckpoint {
    frontend: CheckedFrontend,
    computed_build_config: build_evaluation::ComputedBuildConfig,
    package_authority_verdict:
        Option<crate::package::declaration_admission::AuthoredDeclarationAuthorityVerdict>,
    base_sources: Arc<source::SourceMap>,
}

impl AdmittedBuildCheckpoint {
    fn execute(self) -> Result<ExecutedBuildCheckpoint, Vec<Diagnostic>> {
        let selected_build_symbol = self.admitted_build.selected_build_machine_symbol();
        let computed_build_config = self.admitted_build.execute()?;
        if computed_build_config.selected_build_machine_symbol != selected_build_symbol {
            return Err(vec![Diagnostic::error(
                "build execution returned a selected symbol different from its admitted checkpoint",
            )]);
        }
        Ok(ExecutedBuildCheckpoint {
            frontend: self.frontend,
            computed_build_config,
            package_authority_verdict: self.package_authority_verdict,
            base_sources: self.base_sources,
        })
    }
}

fn lower_checked_frontend(
    mut syntax: source_files_to_assembled_syntax::AssembledSyntax,
    target_name: Option<&str>,
    execution_profile_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<CheckedFrontend, Vec<Diagnostic>> {
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
    // Build-scope sources select their target-scoped declarations against
    // the admitted execution profile; product sources against the target.
    // An unprofiled host resolves to its `NativeTarget` triple.
    let build_scope_sources = std::mem::take(&mut syntax.build_scope_sources);
    let selected_target_machine_declarations =
        build_evaluation::target_machines::filter_target_machines_by_scope(
            &mut syntax.syntax_trees,
            target_name,
            execution_profile_name,
            &build_scope_sources,
        )?;
    let build_source_id = syntax.build_source_id;
    let resolved = syntax_trees_to_symbol_resolved_trees(syntax, timings)?;
    let mut typing_base = symbol_resolved_trees_to_seeded_base(resolved, timings)?;
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
    extension: source_files_to_assembled_syntax::RetainedGeneratedSyntaxExtension,
    selected_target_machine_declarations:
        build_evaluation::target_machines::SelectedTargetMachineDeclarations,
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<
    (
        typed_trees::TypedTrees,
        build_evaluation::target_machines::SelectedTargetMachineDeclarations,
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
    let seeded =
        resolve_seeded_syntax_extension(resolved_base, &extension_syntax, sources, timings)?;
    let rebased = seeded
        .rebase_authored_selections_for_typed_continuation(
            base.typed().authored_declaration_selections(),
        )
        .map_err(|(_, error)| {
            vec![Diagnostic::error(format!(
                "generated-source authored-selection suffix could not join the retained typed base: {error:?}"
            ))]
        })?;
    let mut typed = match type_seeded_extension(rebased, base, timings) {
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
