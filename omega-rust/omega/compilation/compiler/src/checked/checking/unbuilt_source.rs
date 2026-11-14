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
// 05, validation's judgments and Psi 06 never run: the shared source
// preparation returns its frozen owner directly after Psi 04. The caller
// judges the returned frontier with
// `lowered_discharge::judge_compilation`.
//
// A program reaches another package only through a dependency edge, and the
// edge is also a Build decision (`Build::dependency`). The caller states those
// edges as the request's package graph: without one for the standard library,
// `use omega_language_std::Console;` is read as a path inside the program's
// own package and selects nothing. The graph's root package is then the
// validation subject; a source given no graph is its own subject.
//
// Everything else a Build would decide is absent: no Build machine is checked
// or executed, so there are no provider or representation selections, native
// bindings, generated sources or settings, and a provider-default machine is
// kept only when the program reaches it. This is a probe for the route's
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
    child.admit_package_inputs()?;
    let package_inputs = child.package_inputs;
    let core_root = package_compilation::source_imports::bundled_core_root()?;
    let capture = package_compilation::capture_anonymous_compilation_source(&core_root)?;
    let source = source.with_toolchain_capture(capture)?;
    source.admit_root(root)?;
    let (typed, mut timings) = prepare_typed_frontier(source, package_inputs, child.build.profile)?;
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
        constants: compilation_products::BuildConstants::default(),
    };
    let (mut built, build_sources) = build_continuation::complete_build_source(
        typed,
        evaluated,
        package_inputs,
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
        source_completion_accounting,
        ..
    } = complete_prepared_build(
        compilation_products::PreparedBuild {
            built,
            build_sources,
            timings,
        },
        package_inputs,
    )?;
    Ok(frozen_frontier(
        typed,
        source_completion_accounting,
        &computed_build_config.config,
        package_inputs.map(|inputs| inputs.root()),
    )?)
}

/// The product check's own preparation (`source_checking::check_product_source`)
/// for a Build that selected nothing but entries, stopped where it shows the
/// frozen typed trees. `root_package` is the root of the caller's package
/// graph, when it gave one.
fn frozen_frontier(
    typed: typed_trees::TypedTrees,
    accounting: build_time_evaluation::SourceCompletionAccounting,
    build: &compilation_products::BuildConfig,
    root_package: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> Result<TypedFrontier, Vec<Diagnostic>> {
    let layout_scopes = super::source_checking::layout_target_scopes(&typed, build)?;
    let entries = build.program_roots();
    let bindings = super::entry_shapes::bound_program_entry_shapes(&typed, &build.root_bindings)?;
    // The root of a stated graph is the subject. A source given no graph is
    // its own subject: the one package identity its authored product files
    // carry.
    let mut subject = root_package;
    if root_package.is_none()
        && let Some(sources) = typed.symbols.source_map()
    {
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
    // `complete_product_frontend` finishes Psi 03's authored bodies; it has
    // not specialized or frozen them. Run the shared Psi 04 preparation once,
    // exactly as the product path does, and keep the real result. Entries and
    // the subject belong to subsequent structural judgment, not a deleted
    // checker projection or an interrupted source-checking request.
    let prepared = psi_compiler::SourcePreparationRequest {
        program: typed,
        accounting,
        compilation: psi_compiler::TerminalSourceCompilation {
            entries: &entries,
            subjects: &[subject],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &bindings,
            supplies: &[],
        },
        admission: psi_compiler::SourceAdmission {
            external_roots: &[],
            layout_scopes: &layout_scopes,
            layout_selection_authority: None,
            completion: typed_trees::boundary_completions::SourceCompletionAdmission::None,
            representations: psi_compiler::ProductRepresentationInput::default(),
            asm_authority: Some(
                language_core::inline_assembly::AsmAuthorityAdmission::from_freestanding(
                    build.settings.freestanding,
                ),
            ),
            wire_demands: &[],
            declarations: None,
        },
    }
    .prepare()?;
    Ok(TypedFrontier {
        typed: prepared.program.as_ref().clone(),
        source_completion_accounting: prepared.accounting,
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
