// The lowered-form checker ("the route") can be shown a source program
// without running the program's Build and without the old checker.
//
// A project's Build program is itself an Omega program, and an ordinary
// compilation checks, lowers and executes it before the product reaches the
// route. That path belongs to the stages being removed (TASKS.md,
// LOWERED-FORM-CHECKER): once they are gone no Build program can run until the
// route checks and publishes Build programs, every compilation stops at its
// Build, and the people building the route would have no way to show it a
// program.
//
// `typed_frontier_without_build` is that way and nothing more. It types the
// sources as a compilation does (Psi 00 through 03), stands in for the Build
// with the one decision the route needs from it (the named machine of the
// root source, or `main` when none is named and the source declares one, is
// bound to the target's `ProgramEntry`), prepares the product exactly as the
// product check does up to Psi 04's frozen typed trees, and stops there. Psi
// 05, validation's judgments and Psi 06 never run: the preparation is asked to
// stop when it shows the frozen trees, which is the point where the product
// check calls the route. The caller judges the returned frontier with
// `lowered_discharge::judge_compilation`.
//
// What a Build would decide is absent: no Build machine is checked or
// executed, so there are no provider or representation selections, native
// bindings, generated sources, settings or dependencies, and a
// provider-default machine is kept only when the program reaches it. The
// source is its own validation subject. This is a probe for the route's
// developers, not a way to compile: the CLI never calls it and it yields no
// product.

use super::typed_frontier::TypedFrontier;
use super::{build_continuation, complete_prepared_build, prepare_typed_frontier};
use compilation_products::{
    CheckedExecutionRequest, CompileFailure, ComputedBuildConfig, EvaluatedBuild,
    PreparedCheckedSource, PreparedProductSource,
};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{ExecutedRootBinding, RootBindingSelection};
use std::path::Path;
use std::sync::Arc;

pub(in crate::checked) fn typed_frontier_without_build(
    root: &Path,
    entry: Option<&str>,
    source: PreparedCheckedSource,
    child: CheckedExecutionRequest<'_>,
) -> Result<TypedFrontier, CompileFailure> {
    let target = child.selected_target_profile.ok_or_else(|| {
        vec![Diagnostic::error(
            "a program judged without its Build needs a named target, whose `ProgramEntry` the \
             entry machine is bound to",
        )]
    })?;
    if child.package_inputs.is_some() {
        return Err(vec![Diagnostic::error(
            "a program judged without its Build is a standalone source; a package graph is \
             settled by Builds, which this path does not run",
        )]
        .into());
    }
    let core_root = package_compilation::source_imports::bundled_core_root()?;
    let capture = package_compilation::capture_anonymous_compilation_source(&core_root)?;
    let source = source.with_toolchain_capture(capture)?;
    source.admit_root(root)?;
    let (typed, mut timings) = prepare_typed_frontier(source, None, child.build.profile)?;
    let evaluated = EvaluatedBuild {
        configuration: Arc::new(ComputedBuildConfig {
            config: Default::default(),
            root_binding_requests: Vec::new(),
            declared_product_selections: Vec::new(),
            product_selection_requests: Vec::new(),
            native_formations: Vec::new(),
            native_binding_requests: Vec::new(),
            behavior_exclusion_requests: Vec::new(),
            project_declaration: None,
            optimization_report_request: Default::default(),
            evaluation_usage: None,
            observation_summary: None,
            selected_build_machine_symbol: None,
            generated_sources: Vec::new(),
        }),
        restricted_build_requests: Vec::new(),
        base_source_consumption_commitment: None,
        provider_folded_constants: false,
    };
    let (mut built, build_sources) = build_continuation::complete_build_source(
        typed,
        evaluated,
        None,
        child.build.profile,
        child.build.dependency_activation,
        false,
        &mut timings,
    )?;
    // The Product sources are typed only now when the Build source was typed
    // first, so the entry is found here, in the trees the route will judge.
    // A named entry must exist. With none named, `main` is bound when the root
    // source declares it, and a source without one is judged with no entry,
    // as a Build that binds nothing leaves it.
    let binding = match entry {
        Some(entry) => Some(entry_binding(root, entry, target, &built)?),
        None => entry_binding(root, "main", target, &built).ok(),
    };
    Arc::make_mut(&mut built.computed_build_config)
        .root_binding_requests
        .extend(binding);
    let PreparedProductSource {
        typed,
        computed_build_config,
        ..
    } = complete_prepared_build(
        compilation_products::PreparedBuild {
            built,
            build_sources,
            timings,
        },
        None,
    )?;
    Ok(frozen_frontier(
        typed,
        &computed_build_config.config,
        target,
    )?)
}

/// The product check's own preparation (`source_checking::check_product_source`)
/// for a Build that selected nothing but entries, stopped where it shows the
/// frozen typed trees.
fn frozen_frontier(
    typed: typed_trees::TypedTrees,
    build: &compilation_products::BuildConfig,
    target: target::TargetProfile,
) -> Result<TypedFrontier, Vec<Diagnostic>> {
    let layout_scopes = super::source_checking::layout_target_scopes(&typed, build)?;
    let entries = build.program_roots();
    let bindings =
        provider_planning::program_entry::bound_program_entry_shapes(&typed, &build.root_bindings)?;
    let mut demanded = entries.clone();
    demanded.extend(bindings.iter().flat_map(|binding| {
        [
            binding.semantic_schema_symbol,
            binding.requirement_symbol,
            binding.implementation,
        ]
    }));
    demanded.extend(typed_trees::float_providers::catalog_realization_declarations(&typed));
    // A bound entry takes its boundary schema from the target's entry-contract
    // source, which nothing in the program names.
    let entry_contract = (!build.root_bindings.is_empty())
        .then(|| target.program_entry_slot().physical_contract_package)
        .flatten();
    // The standalone source is its own subject: the one package identity its
    // authored product files carry.
    let mut subject = None;
    if let Some(sources) = typed.symbols.source_map() {
        for file in sources.files().filter(|file| {
            file.origin == source::SourceOrigin::User
                && file.dependency_scope == source::DependencyScope::Product
        }) {
            match (subject, file.package_identity) {
                (None, identity) => subject = identity,
                (Some(known), Some(identity)) if known == identity => {}
                _ => {
                    return Err(vec![Diagnostic::error(
                        "a program judged without its Build has authored sources of more than \
                         one package",
                    )]);
                }
            }
        }
    }
    let typed = validation::execution_projection::project_subject_program(
        &typed,
        |file| {
            file.origin == source::SourceOrigin::User
                || entry_contract.is_some_and(|package| {
                    file.path.strip_prefix(&file.package_root).ok()
                        == Some(Path::new(package.package_relative_source()))
                })
        },
        &demanded,
        &[],
    );
    let typed = psi_compiler::prepare_product_typed_frontier(
        typed,
        &psi_compiler::SourceAdmission {
            external_roots: &entries,
            layout_scopes: &layout_scopes,
            layout_selection_authority: None,
            completion: typed_trees::boundary_completions::SourceCompletionAdmission::None,
            representations: psi_compiler::ProductRepresentationInput {
                activation: None,
                declared: &[],
                executed: &[],
                retained: &[],
            },
            asm_authority: Some(validation::AsmAuthorityAdmission::from_freestanding(
                build.settings.freestanding,
            )),
            wire_demands: &[],
            declarations: None,
        },
    )?;
    Ok(TypedFrontier {
        typed,
        entries,
        subject,
        selected: Vec::new(),
        bindings,
    })
}

/// The binding a Build would have executed as
/// `Build::bind(builder, <target>::ProgramEntry, <entry>)`, placed at the
/// entry machine's own declaration in the root source so that the path
/// resolves in the root package as an authored binding does.
fn entry_binding(
    root: &Path,
    entry: &str,
    target: target::TargetProfile,
    built: &compilation_products::BuiltTypedProgram,
) -> Result<ExecutedRootBinding, Vec<Diagnostic>> {
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let typed = built.source.typed();
    let source_span = typed
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str() == entry)
        .filter_map(|machine| typed.symbols.symbol_source_span(machine.symbol))
        .find(|span| {
            built.base_sources.file_at(*span).is_some_and(|file| {
                std::fs::canonicalize(&file.path).unwrap_or_else(|_| file.path.clone()) == root
            })
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "`{}` declares no machine named `{entry}` to bind as the program entry",
                root.display()
            ))]
        })?;
    Ok(ExecutedRootBinding {
        source_span,
        slot: format!(
            "{}::{}",
            target.target_name(),
            target.program_entry_slot().slot_name
        ),
        implementation: RootBindingSelection::AuthoredPath(entry.to_owned()),
    })
}
