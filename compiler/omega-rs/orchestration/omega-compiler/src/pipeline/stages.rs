use crate::pipeline::frontend::{
    discover_imports, extend_source_storage, lex_sources, load_injected_source, load_sources,
    parse_sources, read_bundled_std_source,
};
use crate::pipeline::project::{project_roots, validate_selected_target};
use crate::pipeline::source::{ImportQueue, SourceStorage};
use crate::pipeline::stage::{
    ABSTRACT_OPERATIONS_TO_TARGET_OPERATIONS, ASSIGNED_TARGET_OPERATIONS_TO_MACHINE_INSTRUCTIONS,
    BACKEND_PLAN_TO_NATIVE_IMAGE_PAYLOAD, CHECKED_TREES_TO_STATE_GRAPH,
    CONTROL_FLOW_TO_ABSTRACT_OPERATIONS, SOURCE_FILES_TO_TOKENS, STATE_GRAPH_TO_CONTROL_FLOW,
    SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
    TARGET_OPERATIONS_TO_ASSIGNED_TARGET_OPERATIONS, TOKENS_TO_SYNTAX_TREES,
    TYPED_TREES_TO_CHECKED_TREES,
};
use crate::pipeline::timing::CompileTimings;
use omega_control_flow::ControlFlowPlan;
use omega_emission_planning::{EmissionPlanningInput, build_emission_plan};
use omega_object_file::SectionKind;
use omega_state_graph::StateGraph;
use omega_target::NativeTarget;
use psi_checked_trees::CheckedTrees as CheckedProgram;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_syntax_trees::SyntaxTrees;
use psi_typed_trees::TypedTrees;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(super) struct AssembledSyntax {
    pub(super) syntax_trees: SyntaxTrees,
    pub(super) files: Vec<crate::pipeline::source::SourceFile>,
    pub(super) sources: Arc<psi_source::SourceMap>,
}

pub(super) struct CheckedProgramSurface {
    pub(super) program: Arc<CheckedProgram>,
    pub(super) selected_provider_plans: Arc<omega_effects::SelectedProviderPlanFacts>,
    pub(super) task_activations: Arc<omega_task_plans::TaskActivationPlanSet>,
}

pub(super) struct BackendPlanningSurface {
    pub(super) plan: omega_backend_plan::BackendPlan,
}

pub(super) struct EmittedProgram {
    pub(super) target: NativeTarget,
    /// PE optional-header Subsystem resolved from the selected target's
    /// `subsystem <word>` (console 3 by default). The PE writer stamps it into
    /// the image; the Mach-O output path translates gui (2) into an `.app`
    /// bundle beside the flat binary; other formats ignore it.
    pub(super) subsystem: u16,
    pub(super) planned_text_bytes: usize,
    pub(super) object: omega_object_file::ObjectPlan,
    pub(super) relocations: omega_object_file::RelocationPlan,
    pub(super) encoded_machine_code: omega_machine_bytes::EncodedMachineCode,
    pub(super) encoded_machine_semantics: omega_machine_bytes::EncodedMachineSemanticSummary,
    pub(super) text_bytes: Vec<u8>,
    pub(super) data_bytes: Vec<u8>,
}

pub(super) fn source_files_to_syntax_trees(
    root_path: &Path,
    target_name: Option<&str>,
    timings: &mut CompileTimings,
) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
    // `native: true` — the full native-image path substitutes target-specific
    // providers (task #57: `gui: Gui` -> `MacosGui` on darwin). The interpreter's
    // `compile_to_checked` path keeps the abstract boundary traits (its own headless
    // stubs), so it calls the `..._for_engine(false)` variant.
    source_files_to_syntax_trees_for_engine(root_path, target_name, true, timings)
}

pub(super) fn source_files_to_syntax_trees_for_engine(
    root_path: &Path,
    target_name: Option<&str>,
    native: bool,
    timings: &mut CompileTimings,
) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
    let mut imports = ImportQueue::default();
    for root in project_roots(root_path) {
        imports.seed(root);
    }

    let root_package = root_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut source_storage = SourceStorage::for_compilation(
        root_package,
        crate::pipeline::frontend::bundled_omega_root(),
    );
    // depend-mapping (M2 blocker 3): `b.depend("alias", path("dir"))` rows
    // collected from every loaded build machine, alias -> directory. Each
    // frontier collects BEFORE resolving its uses, so a build.omg companion
    // maps aliases for the sources loaded alongside it.
    let mut depend_aliases: Vec<(String, PathBuf)> = Vec::new();

    load_pending_imports(
        &mut source_storage,
        &mut imports,
        root_path,
        target_name,
        &mut depend_aliases,
        timings,
    )?;

    inject_build_prelude(&mut source_storage, timings)?;

    if native {
        substitute_native_gui_provider(
            &mut source_storage,
            root_path,
            target_name,
            &mut imports,
            &mut depend_aliases,
            timings,
        )?;
    }

    validate_selected_target(&source_storage, target_name)?;
    let source_file_count = source_storage.file_count();
    let syntax = assemble_syntax(source_storage)?;

    Ok((source_file_count, syntax))
}

fn load_pending_imports(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    root_path: &Path,
    target_name: Option<&str>,
    depend_aliases: &mut Vec<(String, PathBuf)>,
    timings: &mut CompileTimings,
) -> Result<(), Vec<Diagnostic>> {
    while imports.has_pending() {
        let frontier = imports.take_frontier();
        let first_source_id = source_storage.next_source_id();
        let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
            let sources = load_sources(frontier, first_source_id)?;
            lex_sources(sources)
        })?;
        let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
            parse_sources(lexed, &mut source_storage.syntax_trees)
        })?;
        crate::pipeline::frontend::collect_depend_aliases(
            &parsed,
            &source_storage.syntax_trees,
            depend_aliases,
        );
        for (_, directory) in depend_aliases.iter() {
            source_storage.register_package_root(directory.clone());
        }
        let discovered_imports = discover_imports(
            &parsed,
            &source_storage.syntax_trees,
            root_path,
            target_name,
            depend_aliases,
        )?;

        imports.enqueue(discovered_imports)?;
        extend_source_storage(source_storage, parsed)?;
    }

    Ok(())
}

/// The TOOLCHAIN-PROVIDED build vocabulary (build_and_package_model.md): a
/// build.omg is just `machine build(b: &mut Build) { ... }` or the scoped
/// `machine Owner::build(&mut self, b: &mut Build) { ... }` -- the `Build` /
/// `Subsystem` types are CORE-DEFINED, never authored per file. When a
/// build.omg root declares either build-machine shape and no `Build` data of
/// its own, the prelude is injected as a virtual source (a program-declared
/// `Build` wins, which keeps migration and deliberate overrides possible).
const BUILD_PRELUDE: &str = r#"
// Toolchain-provided build vocabulary (virtual source; build_and_package_model.md).
data Subsystem {
    case Console;
    case Gui;
    case EfiApplication;
    case Unspecified(value: u16);
}
data Build {
    subsystem: Subsystem;
    freestanding: bool;
}
machine Build::depend(&mut self, alias: &[u8], location: &[u8]) {
}
machine path(location: &[u8]) -> &[u8] {
    transition { _ -> (location) }
}
"#;

fn inject_build_prelude(
    source_storage: &mut SourceStorage,
    timings: &mut CompileTimings,
) -> Result<(), Vec<Diagnostic>> {
    let mut has_build_machine = false;
    let mut has_build_data = false;
    for (_, file) in source_storage.files.iter() {
        let is_build_file =
            file.path.file_name().and_then(|name| name.to_str()) == Some("build.omg");
        for root_item in &file.root_items {
            match source_storage.syntax_trees.root_item(*root_item) {
                psi_syntax_trees::item::Item::Machine(machine)
                    if is_build_file
                        && (machine.name.as_str() == "build"
                            || machine.name.as_str().ends_with("::build")) =>
                {
                    has_build_machine = true;
                }
                psi_syntax_trees::item::Item::Data(data) if data.name.as_str() == "Build" => {
                    has_build_data = true;
                }
                _ => {}
            }
        }
    }
    if !has_build_machine || has_build_data {
        return Ok(());
    }

    let first_source_id = source_storage.next_source_id();
    let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
        let sources = crate::pipeline::frontend::load_injected_source(
            "<build-prelude>",
            BUILD_PRELUDE,
            first_source_id,
        );
        lex_sources(sources)
    })?;
    let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
        parse_sources(lexed, &mut source_storage.syntax_trees)
    })?;
    extend_source_storage(source_storage, parsed)?;
    Ok(())
}

/// The darwin boundary-provider substitution (tasks #57/#60). The samples call the
/// UNCHANGED Win32-shaped `boundary trait Gui` / `boundary trait Input` through
/// `gui: Gui` / `input: Input` fields. On darwin there is no host lowering for these;
/// instead the bundled `omega::language::std::macos_gui` module provides `MacosGui` /
/// `MacosInput` data types that implement every op by composing objc / Core Graphics
/// boundary calls. This (1) injects that provider module (which the sample never
/// `use`s) and (2) rewrites each matching FIELD's type to its provider, so
/// `self.gui.op(..)` / `self.input.op(..)` become ordinary provider value-calls — the
/// shape proven to run natively (window_demo, fire 25). NATIVE-only: the interpreter's
/// `compile_to_checked` keeps the abstract boundary traits (its own headless stubs,
/// item #9). Registry-driven so Clock and future providers follow the same pattern.
const DARWIN_BOUNDARY_PROVIDERS: &[(&str, &str)] = &[("Gui", "MacosGui"), ("Input", "MacosInput")];

fn substitute_native_gui_provider(
    source_storage: &mut SourceStorage,
    root_path: &Path,
    target_name: Option<&str>,
    imports: &mut ImportQueue,
    depend_aliases: &mut Vec<(String, PathBuf)>,
    timings: &mut CompileTimings,
) -> Result<(), Vec<Diagnostic>> {
    // Gate strictly on darwin/Mach-O: on Windows `Gui`/`Input` have real Win32 host
    // lowerings (do NOT substitute); on linux they have none (a clean diagnostic).
    let is_darwin = NativeTarget::from_omega_target_name(target_name)
        .map(|target| target.object_format == omega_target::ObjectFormat::MachO)
        .unwrap_or(false);
    if !is_darwin {
        return Ok(());
    }

    // Which registered `boundary trait`s does the program declare, and is the provider
    // module already present? (`MacosGui` names the bundled module; it carries both
    // MacosGui + MacosInput.)
    let mut has_any_boundary = false;
    let mut has_provider_module = false;
    for (_, file) in source_storage.files.iter() {
        for root_item in &file.root_items {
            match source_storage.syntax_trees.root_item(*root_item) {
                psi_syntax_trees::item::Item::Trait(trait_definition)
                    if trait_definition.is_boundary
                        && DARWIN_BOUNDARY_PROVIDERS
                            .iter()
                            .any(|(boundary, _)| *boundary == trait_definition.name.as_str()) =>
                {
                    has_any_boundary = true;
                }
                psi_syntax_trees::item::Item::Data(data) if data.name.as_str() == "MacosGui" => {
                    has_provider_module = true;
                }
                _ => {}
            }
        }
    }
    if !has_any_boundary {
        return Ok(());
    }

    // Inject the bundled provider module, then load its ordinary dependency
    // closure. Injection happens after the root import queue is exhausted, so
    // simply parsing its `use` declarations is not enough: newly discovered
    // imports must re-enter the same queue and recursive loader as authored
    // sources. This became observable when MacosGui adopted the public numeric
    // conversion machines.
    if !has_provider_module {
        let provider_source = read_bundled_std_source("macos_gui")?;
        let first_source_id = source_storage.next_source_id();
        let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
            let sources =
                load_injected_source("<macos-gui-provider>", &provider_source, first_source_id);
            lex_sources(sources)
        })?;
        let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
            parse_sources(lexed, &mut source_storage.syntax_trees)
        })?;
        crate::pipeline::frontend::collect_depend_aliases(
            &parsed,
            &source_storage.syntax_trees,
            depend_aliases,
        );
        for (_, directory) in depend_aliases.iter() {
            source_storage.register_package_root(directory.clone());
        }
        let discovered_imports = discover_imports(
            &parsed,
            &source_storage.syntax_trees,
            root_path,
            target_name,
            depend_aliases,
        )?;
        imports.enqueue(discovered_imports)?;
        extend_source_storage(source_storage, parsed)?;
        load_pending_imports(
            source_storage,
            imports,
            root_path,
            target_name,
            depend_aliases,
            timings,
        )?;
    }

    // Rewrite each `<field>: <Boundary>` FIELD -> its provider data type. Collect first
    // (immutable borrows), then mutate, exactly like the plan-laid value-type desugar.
    let mut rewrites: Vec<(psi_syntax_trees::types::TypeReferenceHandle, &'static str)> =
        Vec::new();
    for (_, file) in source_storage.files.iter() {
        for root_item in &file.root_items {
            let psi_syntax_trees::item::Item::Data(definition) =
                source_storage.syntax_trees.root_item(*root_item)
            else {
                continue;
            };
            let members = definition.members;
            for member in source_storage
                .syntax_trees
                .tables
                .items
                .data_members(members)
            {
                let psi_syntax_trees::item::DataMember::Field(field) = member else {
                    continue;
                };
                if let psi_syntax_trees::types::TypeReferenceNode::Named(name) = source_storage
                    .syntax_trees
                    .tables
                    .type_references
                    .type_reference(field.type_reference)
                    && let Some((_, provider)) = DARWIN_BOUNDARY_PROVIDERS
                        .iter()
                        .find(|(boundary, _)| *boundary == name.as_str())
                {
                    rewrites.push((field.type_reference, provider));
                }
            }
        }
    }
    for (handle, provider) in rewrites {
        source_storage
            .syntax_trees
            .tables
            .type_references
            .replace_type_reference(
                handle,
                psi_syntax_trees::types::TypeReferenceNode::Named(
                    psi_syntax_trees::identifier::Identifier::generated(provider.to_string()),
                ),
            );
    }

    Ok(())
}

fn assemble_syntax(sources: SourceStorage) -> Result<AssembledSyntax, Vec<Diagnostic>> {
    let files = sources.files.storage_slice().to_vec();
    Ok(AssembledSyntax {
        syntax_trees: sources.syntax_trees,
        files,
        sources: Arc::new(sources.sources),
    })
}

pub(super) fn syntax_trees_to_symbol_resolved_trees(
    syntax: AssembledSyntax,
    timings: &mut CompileTimings,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    timings.record(SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES, || {
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
            &syntax.syntax_trees,
            syntax.sources,
        )
        .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn symbol_resolved_trees_to_typed_trees(
    resolved: SymbolResolvedTrees,
    timings: &mut CompileTimings,
) -> Result<TypedTrees, Vec<Diagnostic>> {
    timings.record(SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, || {
        psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_owned(resolved)
            .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn typed_trees_to_checked_trees(
    typed: TypedTrees,
    timings: &mut CompileTimings,
) -> Result<CheckedProgramSurface, Vec<Diagnostic>> {
    timings.record(TYPED_TREES_TO_CHECKED_TREES, || {
        let program = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)?;
        crate::pipeline::provider_approval::check_boundary_provider_approval(&program)?;
        Ok(CheckedProgramSurface {
            program: Arc::new(program),
            selected_provider_plans: Arc::new(omega_effects::SelectedProviderPlanFacts::default()),
            task_activations: Arc::new(omega_task_plans::TaskActivationPlanSet::default()),
        })
    })
}

pub(super) fn checked_trees_to_state_graph(
    checked: &CheckedProgramSurface,
    workers: omega_core::parallel::WorkerPoolHandle,
    timings: &mut CompileTimings,
) -> Result<StateGraph, Vec<Diagnostic>> {
    timings.record(CHECKED_TREES_TO_STATE_GRAPH, || {
        omega_checked_trees_to_state_graph::build_state_graph_with_workers(
            Arc::clone(&checked.program),
            workers,
        )
        .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn state_graph_to_control_flow(
    state_graph: StateGraph,
    timings: &mut CompileTimings,
) -> Result<ControlFlowPlan, Vec<Diagnostic>> {
    timings.record(STATE_GRAPH_TO_CONTROL_FLOW, || {
        omega_state_graph_to_control_flow::build_control_flow_plan_owned(state_graph)
            .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn control_flow_to_backend_plan(
    checked: CheckedProgramSurface,
    entry_machine_name: Option<&str>,
    target_name: Option<&str>,
    freestanding: bool,
    external_binding_rows: &[omega_calling_conventions::ExternalBindingRow],
    control_flow: ControlFlowPlan,
    workers: omega_core::parallel::WorkerPoolHandle,
    timings: &mut CompileTimings,
) -> Result<BackendPlanningSurface, Vec<Diagnostic>> {
    let target =
        NativeTarget::from_omega_target_name(target_name).map_err(|diagnostic| vec![diagnostic])?;

    let plan = omega_backend_pipeline::build_backend_plan_from_control_flow_with_workers(
        checked.program,
        checked.selected_provider_plans,
        entry_machine_name,
        target,
        freestanding,
        external_binding_rows,
        Arc::new(control_flow),
        workers,
    )
    .map_err(|diagnostic| vec![diagnostic])?;

    record_backend_phase_as_stage(
        timings,
        &plan,
        "abstract operations",
        CONTROL_FLOW_TO_ABSTRACT_OPERATIONS,
    )?;
    record_backend_phase_as_stage(
        timings,
        &plan,
        "target operations",
        ABSTRACT_OPERATIONS_TO_TARGET_OPERATIONS,
    )?;
    record_backend_phase_as_stage(
        timings,
        &plan,
        "assigned target operations",
        TARGET_OPERATIONS_TO_ASSIGNED_TARGET_OPERATIONS,
    )?;
    record_backend_phase_as_stage(
        timings,
        &plan,
        "machine instructions",
        ASSIGNED_TARGET_OPERATIONS_TO_MACHINE_INSTRUCTIONS,
    )?;

    Ok(BackendPlanningSurface { plan })
}

fn plan_emission(plan: &omega_backend_plan::BackendPlan) -> omega_artifacts::EmissionPlan {
    build_emission_plan(&EmissionPlanningInput {
        receiver_bases: &plan.receiver_bases,
        state_contexts: &plan.state_contexts,
        target: plan.target,
        entry_key: plan.entry_key,
        host_abi: &plan.host_abi,
        host_calls: &plan.host_calls,
        state_calls: &plan.state_calls,
        state_storage: &plan.state_storage,
        state_values: &plan.state_values,
        data: &plan.data,
        instructions: &plan.target_operations,
        control_flow: &plan.control_flow,
        runtime_flow: &plan.runtime_flow,
        runtime_bodies: &plan.runtime_bodies,
        runtime_branching_calls: &plan.runtime_branching_calls,
        runtime_dispatch_loop: &plan.runtime_dispatch_loop,
        runtime_storage: &plan.runtime_storage,
        runtime_text: &plan.runtime_text,
        state_guards: &plan.state_guards,
        layouts: &plan.layouts,
        machine_instructions: &plan.machine_instructions,
        encoded_machine: &plan.encoded_machine,
        object: &plan.object,
        relocations: &plan.relocations,
    })
}

pub(super) fn ensure_emission_ready(
    emission_plan: &omega_artifacts::EmissionPlan,
) -> Result<(), Vec<Diagnostic>> {
    if emission_plan.blockers.is_empty() {
        return Ok(());
    }

    Err(emission_plan
        .blockers
        .iter()
        .map(|(_, blocker)| Diagnostic::error(format!("{}: {}", blocker.stage, blocker.reason)))
        .collect())
}

pub(super) fn backend_plan_to_native_image_payload(
    backend: &BackendPlanningSurface,
    subsystem: u16,
    timings: &mut CompileTimings,
) -> Result<(omega_artifacts::EmissionPlan, EmittedProgram), Vec<Diagnostic>> {
    timings.record(BACKEND_PLAN_TO_NATIVE_IMAGE_PAYLOAD, || {
        let emission_plan = plan_emission(&backend.plan);
        ensure_emission_ready(&emission_plan)?;
        let plan = &backend.plan;
        let text_bytes = plan.encoded_machine.code.bytes.storage_slice().to_vec();
        let emitted = EmittedProgram {
            target: plan.target,
            subsystem,
            planned_text_bytes: object_text_size(&plan.object),
            object: plan.object.clone(),
            relocations: plan.relocations.clone(),
            encoded_machine_code: plan.encoded_machine.code.clone(),
            encoded_machine_semantics: plan.encoded_machine.semantics.clone(),
            text_bytes,
            data_bytes: plan.data.bytes.storage_slice().to_vec(),
        };
        Ok((emission_plan, emitted))
    })
}

fn object_text_size(object: &omega_object_file::ObjectPlan) -> usize {
    object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Text)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

fn record_backend_phase_as_stage(
    timings: &mut CompileTimings,
    plan: &omega_backend_plan::BackendPlan,
    backend_phase: &str,
    stage: crate::pipeline::stage::StageMeta,
) -> Result<(), Vec<Diagnostic>> {
    let Some((_, phase_timing)) = plan
        .phase_timings
        .iter()
        .find(|(_, phase_timing)| phase_timing.phase == backend_phase)
    else {
        return Err(vec![Diagnostic::error(format!(
            "backend phase `{backend_phase}` was not recorded for {}",
            stage.label()
        ))]);
    };

    timings.add_completed(
        stage,
        phase_timing.microseconds,
        phase_timing.allocations.clone(),
    );
    Ok(())
}
