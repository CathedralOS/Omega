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
use omega_checked_trees::CheckedTrees as CheckedProgram;
use omega_control_flow::ControlFlowPlan;
use omega_core::diagnostics::Diagnostic;
use omega_emission_planning::{EmissionPlanningInput, build_emission_plan};
use omega_object_file::SectionKind;
use omega_state_graph::StateGraph;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_syntax_trees::SyntaxTrees;
use omega_target::NativeTarget;
use omega_typed_trees::TypedTrees;
use std::path::Path;
use std::sync::Arc;

pub(super) struct AssembledSyntax {
    pub(super) syntax_trees: SyntaxTrees,
    pub(super) files: Vec<crate::pipeline::source::SourceFile>,
    pub(super) sources: Arc<omega_core::source::SourceMap>,
}

pub(super) struct CheckedProgramSurface {
    pub(super) program: Arc<CheckedProgram>,
}

pub(super) struct BackendPlanningSurface {
    pub(super) plan: omega_backend_plan::BackendPlan,
}

pub(super) struct EmittedProgram {
    pub(super) target: NativeTarget,
    /// PE optional-header Subsystem resolved from the selected target's
    /// `subsystem <word>` (console 3 by default); non-PE formats ignore it.
    pub(super) subsystem: u16,
    pub(super) planned_text_bytes: usize,
    pub(super) object: omega_object_file::ObjectPlan,
    pub(super) relocations: omega_object_file::RelocationPlan,
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

    let mut source_storage = SourceStorage::default();

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
        let discovered_imports = discover_imports(
            &parsed,
            &source_storage.syntax_trees,
            root_path,
            target_name,
        )?;

        imports.enqueue(discovered_imports)?;
        extend_source_storage(&mut source_storage, parsed)?;
    }

    inject_build_prelude(&mut source_storage, timings)?;

    if native {
        substitute_native_gui_provider(&mut source_storage, target_name, timings)?;
    }

    validate_selected_target(&source_storage, target_name)?;
    let source_file_count = source_storage.file_count();
    let syntax = assemble_syntax(source_storage)?;

    Ok((source_file_count, syntax))
}

/// The TOOLCHAIN-PROVIDED build vocabulary (build_and_package_model.md): a
/// build.omg is just `machine build(b: &mut Build) { ... }` -- the `Build` /
/// `Subsystem` types are CORE-DEFINED, never authored per file. When the
/// program declares a `build` machine and no `Build` data of its own, the
/// prelude is injected as a virtual source (a program-declared `Build` wins,
/// which keeps migration and deliberate overrides possible).
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
"#;

fn inject_build_prelude(
    source_storage: &mut SourceStorage,
    timings: &mut CompileTimings,
) -> Result<(), Vec<Diagnostic>> {
    let mut has_build_machine = false;
    let mut has_build_data = false;
    for (_, file) in source_storage.files.iter() {
        for root_item in &file.root_items {
            match source_storage.syntax_trees.root_item(*root_item) {
                omega_syntax_trees::item::Item::Machine(machine)
                    if machine.name.as_str() == "build" =>
                {
                    has_build_machine = true;
                }
                omega_syntax_trees::item::Item::Data(data) if data.name.as_str() == "Build" => {
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

/// The darwin GUI-provider substitution (task #57). The samples call the UNCHANGED
/// `boundary trait Gui` (Win32-shaped) through a `gui: Gui` field. On darwin there is
/// no host lowering for `Gui`; instead the bundled `MacosGui` data type
/// (`omega::language::std::macos_gui`) implements every op by composing objc / Core
/// Graphics boundary calls. This (1) injects that provider module (which the sample
/// never `use`s) and (2) rewrites each `gui: Gui` FIELD's type to `MacosGui`, so
/// `self.gui.op(..)` becomes an ordinary MacosGui value-call — the shape proven to run
/// natively in fire 22. NATIVE-only: the interpreter's `compile_to_checked` keeps the
/// abstract boundary trait (its own headless stub, item #9). Registry-shaped so
/// Clock/Input providers can follow the same pattern.
fn substitute_native_gui_provider(
    source_storage: &mut SourceStorage,
    target_name: Option<&str>,
    timings: &mut CompileTimings,
) -> Result<(), Vec<Diagnostic>> {
    // Gate strictly on darwin/Mach-O: on Windows `Gui` has a real Win32 host lowering
    // (do NOT substitute); on linux it has none (a clean unsupported diagnostic).
    let is_darwin = NativeTarget::from_omega_target_name(target_name)
        .map(|target| target.object_format == omega_target::ObjectFormat::MachO)
        .unwrap_or(false);
    if !is_darwin {
        return Ok(());
    }

    // Detect the sample's `boundary trait Gui` and whether the provider is present.
    let mut has_gui_boundary = false;
    let mut has_macos_gui = false;
    for (_, file) in source_storage.files.iter() {
        for root_item in &file.root_items {
            match source_storage.syntax_trees.root_item(*root_item) {
                omega_syntax_trees::item::Item::Trait(trait_definition)
                    if trait_definition.is_boundary
                        && trait_definition.name.as_str() == "Gui" =>
                {
                    has_gui_boundary = true;
                }
                omega_syntax_trees::item::Item::Data(data) if data.name.as_str() == "MacosGui" => {
                    has_macos_gui = true;
                }
                _ => {}
            }
        }
    }
    if !has_gui_boundary {
        return Ok(());
    }

    // Inject the bundled MacosGui provider module (self-contained; declares its own
    // ObjectiveC/CoreGraphics boundary traits + CString domain, no `use`).
    if !has_macos_gui {
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
        extend_source_storage(source_storage, parsed)?;
    }

    // Rewrite each `gui: Gui`-typed FIELD -> `MacosGui`. Collect first (immutable
    // borrows), then mutate, exactly like the plan-laid value-type desugar.
    let mut rewrites: Vec<omega_syntax_trees::types::TypeReferenceHandle> = Vec::new();
    for (_, file) in source_storage.files.iter() {
        for root_item in &file.root_items {
            let omega_syntax_trees::item::Item::Data(definition) =
                source_storage.syntax_trees.root_item(*root_item)
            else {
                continue;
            };
            let members = definition.members;
            for member in source_storage.syntax_trees.tables.items.data_members(members) {
                let omega_syntax_trees::item::DataMember::Field(field) = member else {
                    continue;
                };
                if let omega_syntax_trees::types::TypeReferenceNode::Named(name) = source_storage
                    .syntax_trees
                    .tables
                    .type_references
                    .type_reference(field.type_reference)
                    && name.as_str() == "Gui"
                {
                    rewrites.push(field.type_reference);
                }
            }
        }
    }
    for handle in rewrites {
        source_storage
            .syntax_trees
            .tables
            .type_references
            .replace_type_reference(
                handle,
                omega_syntax_trees::types::TypeReferenceNode::Named(
                    omega_syntax_trees::identifier::Identifier::generated("MacosGui".to_string()),
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
        omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
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
        omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_owned(resolved)
            .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn typed_trees_to_checked_trees(
    typed: TypedTrees,
    timings: &mut CompileTimings,
) -> Result<CheckedProgramSurface, Vec<Diagnostic>> {
    timings.record(TYPED_TREES_TO_CHECKED_TREES, || {
        let program = omega_typed_trees_to_checked_trees::lower_typed_trees(typed)?;
        Ok(CheckedProgramSurface {
            program: Arc::new(program),
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
    target_name: Option<&str>,
    freestanding: bool,
    provides_rows: &[omega_calling_conventions::ProvidesRow],
    control_flow: ControlFlowPlan,
    workers: omega_core::parallel::WorkerPoolHandle,
    timings: &mut CompileTimings,
) -> Result<BackendPlanningSurface, Vec<Diagnostic>> {
    let target =
        NativeTarget::from_omega_target_name(target_name).map_err(|diagnostic| vec![diagnostic])?;

    let plan = omega_backend_pipeline::build_backend_plan_from_control_flow_with_workers(
        checked.program,
        target,
        freestanding,
        provides_rows,
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
