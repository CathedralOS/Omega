use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::compile_report::CompileReport;
use crate::pipeline::frontend::{
    discover_imports, extend_source_storage, lex_sources, load_sources, parse_sources,
};
use crate::pipeline::source::{ImportQueue, SourceStorage};
use omega_artifacts::{ArtifactWriter, build_backend_surface_report};
use omega_backend_report::{BackendReportInput, BackendReportPhaseTiming, backend_report_text};
use omega_checked_trees::Program as CheckedProgram;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPool;
use omega_emission_planning::{EmissionPlanningInput, build_emission_plan};
use omega_image_emission::{ExecutableImageInput, can_emit_executable_image, emit_checked_executable_image};
use omega_object::{ObjectContainerInput, SectionKind, emit_omega_object_container};
use omega_resolved_trees::Program as ResolvedProgram;
use omega_syntax_trees::SyntaxTrees;
use omega_target::NativeTarget;
use omega_typed_trees::Program as TypedProgram;

pub fn compile(options: CompileOptions) -> Result<CompileReport, Vec<Diagnostic>> {
    Compiler::new(options).compile()
}

pub struct Compiler {
    options: CompileOptions,
}

impl Compiler {
    pub fn new(options: CompileOptions) -> Self {
        Self { options }
    }

    pub fn compile(self) -> Result<CompileReport, Vec<Diagnostic>> {
        let mut imports = ImportQueue::default();
        for root in project_roots(&self.options.root_path) {
            imports.seed(root);
        }
        let workers = WorkerPool::with_available_parallelism();

        let mut source_storage = SourceStorage::default();

        while imports.has_pending() {
            let frontier = imports.take_frontier();
            let first_source_id = source_storage.next_source_id();
            let sources = load_sources(frontier, first_source_id)?;
            let lexed = lex_sources(sources)?;
            let parsed = parse_sources(lexed)?;
            let discovered_imports =
                discover_imports(&parsed, &self.options.root_path, self.options.target_name.as_deref())?;

            imports.enqueue(discovered_imports)?;
            extend_source_storage(&mut source_storage, parsed)?;
        }

        validate_selected_target(&source_storage, self.options.target_name.as_deref())?;

        let syntax = assemble_syntax(&source_storage)?;
        let resolved = resolve_program(syntax)?;
        let typed = typecheck_program(resolved)?;
        let checked = check_program(&typed)?;
        let backend_surface = build_backend_surface_report(&checked.program);
        let planned = plan_backend(
            checked,
            self.options.target_name.as_deref(),
            workers.handle(),
        )?;
        let emission_plan = plan_emission(&planned);
        if self.options.write_output {
            write_backend_report(&self.options, &backend_surface, &planned)?;
            write_emission_plan(&self.options, &emission_plan)?;
        }
        ensure_emission_ready(&emission_plan)?;
        let emitted = emit_backend(&planned)?;

        if self.options.write_output {
            write_output(&self.options, emitted)?;
        }

        Ok(CompileReport {
            root_path: self.options.root_path,
            source_file_count: source_storage.file_count(),
            wrote_output: self.options.write_output,
        })
    }
}

fn project_roots(root_path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut roots = vec![root_path.to_path_buf()];
    let Some(parent) = root_path.parent() else {
        return roots;
    };

    for companion_name in companion_root_names(root_path.file_name().and_then(|name| name.to_str())) {
        let companion = parent.join(companion_name);
        if companion != root_path && companion.is_file() {
            roots.push(companion);
        }
    }

    roots
}

fn companion_root_names(root_name: Option<&str>) -> &'static [&'static str] {
    match root_name {
        Some("main.omg") => &["build.omg"],
        Some("build.omg") => &["main.omg"],
        _ => &["build.omg"],
    }
}

fn validate_selected_target(
    source_storage: &SourceStorage,
    selected_target_name: Option<&str>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(target_name) = selected_target_name else {
        return Ok(());
    };

    let target_found = source_storage.files.iter().any(|(_, file)| {
        file.syntax_trees.root_items().any(|item| {
            matches!(
                item,
                omega_syntax_trees::item::Item::Target(target) if target.name.as_str() == target_name
            )
        })
    });

    if target_found {
        return Ok(());
    }

    Err(vec![Diagnostic::error(format!(
        "target `{target_name}` was not found in discovered source frontier"
    ))])
}

struct AssembledSyntax {
    syntax_trees: SyntaxTrees,
}

struct CheckedProgramSurface {
    program: CheckedProgram,
}

struct EmittedProgram {
    target: NativeTarget,
    planned_text_bytes: usize,
    object: omega_object::ObjectPlan,
    relocations: omega_object::RelocationPlan,
    text_bytes: Vec<u8>,
    data_bytes: Vec<u8>,
}

fn assemble_syntax(_sources: &SourceStorage) -> Result<AssembledSyntax, Vec<Diagnostic>> {
    let mut syntax_trees = SyntaxTrees::new(Default::default());

    for (_, file) in _sources.files.iter() {
        for item in file.syntax_trees.root_items() {
            syntax_trees.push_root_item(item.clone());
        }
    }

    Ok(AssembledSyntax {
        syntax_trees,
    })
}

fn resolve_program(syntax: AssembledSyntax) -> Result<ResolvedProgram, Vec<Diagnostic>> {
    omega_syntax_trees_to_resolved_trees::lower_syntax_trees(&syntax.syntax_trees)
        .map_err(|diagnostic| vec![diagnostic])
}

fn typecheck_program(resolved: ResolvedProgram) -> Result<TypedProgram, Vec<Diagnostic>> {
    omega_resolved_trees_to_typed_trees::lower_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])
}

fn check_program(typed: &TypedProgram) -> Result<CheckedProgramSurface, Vec<Diagnostic>> {
    let program = omega_typed_trees_to_checked_trees::lower_typed_trees(typed)?;
    Ok(CheckedProgramSurface { program })
}

fn plan_backend(
    checked: CheckedProgramSurface,
    target_name: Option<&str>,
    workers: omega_core::parallel::WorkerPoolHandle,
) -> Result<omega_backend_plan::BackendPlan, Vec<Diagnostic>> {
    let target =
        NativeTarget::from_omega_target_name(target_name).map_err(|diagnostic| vec![diagnostic])?;
    let checked_program = std::sync::Arc::new(checked.program);
    let state_graph = omega_checked_trees_to_state_graph::build_state_graph_with_workers(
        std::sync::Arc::clone(&checked_program),
        workers.clone(),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let control_flow =
        omega_state_graph_to_control_flow::build_control_flow_plan(&state_graph)
            .map_err(|diagnostic| vec![diagnostic])?;

    omega_backend_pipeline::build_backend_plan_from_control_flow_with_workers(
        checked_program,
        target,
        std::sync::Arc::new(control_flow),
        workers,
    )
    .map_err(|diagnostic| vec![diagnostic])
}

fn emit_backend(plan: &omega_backend_plan::BackendPlan) -> Result<EmittedProgram, Vec<Diagnostic>> {
    let text_bytes = plan.encoded_machine.bytes.storage_slice().to_vec();
    Ok(EmittedProgram {
        target: plan.target,
        planned_text_bytes: object_text_size(&plan.object),
        object: plan.object.clone(),
        relocations: plan.relocations.clone(),
        text_bytes,
        data_bytes: plan.data.bytes.storage_slice().to_vec(),
    })
}

fn write_backend_report(
    options: &CompileOptions,
    backend_surface: &omega_artifacts::BackendSurfaceReport,
    plan: &omega_backend_plan::BackendPlan,
) -> Result<(), Vec<Diagnostic>> {
    let phase_timings = plan
        .phase_timings
        .iter()
        .map(|timing| BackendReportPhaseTiming {
            phase: timing.phase.clone(),
            microseconds: timing.microseconds,
            allocations: timing.allocations,
        })
        .collect::<Vec<_>>();
    let report = backend_report_text(
        backend_surface,
        &BackendReportInput {
            target: plan.target,
            entry_key: plan.entry_key,
            phase_timings: &phase_timings,
            host_abi: &plan.host_abi,
            host_calls: &plan.host_calls,
            state_calls: &plan.state_calls,
            alias_flow: &plan.alias_flow,
            state_storage: &plan.state_storage,
            state_values: &plan.state_values,
            data: &plan.data,
            instructions: &plan.instructions,
            control_flow: &plan.control_flow,
            runtime_flow: &plan.runtime_flow,
            state_dispatch: &plan.state_dispatch,
            state_guards: &plan.state_guards,
            runtime_bodies: &plan.runtime_bodies,
            runtime_branching_calls: &plan.runtime_branching_calls,
            runtime_dispatch_loop: &plan.runtime_dispatch_loop,
            runtime_storage: &plan.runtime_storage,
            runtime_text: &plan.runtime_text,
            layouts: &plan.layouts,
            machine_program: &plan.machine_program,
            encoded_machine: &plan.encoded_machine,
            object: &plan.object,
            relocations: &plan.relocations,
        },
    );

    let build_dir = options.build_dir();
    let output_path = build_dir.join("09_backend_report.txt");
    std::fs::create_dir_all(build_dir).map_err(io_diagnostic)?;
    std::fs::write(output_path, report).map_err(io_diagnostic)
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
        instructions: &plan.instructions,
        control_flow: &plan.control_flow,
        runtime_flow: &plan.runtime_flow,
        runtime_bodies: &plan.runtime_bodies,
        runtime_branching_calls: &plan.runtime_branching_calls,
        runtime_dispatch_loop: &plan.runtime_dispatch_loop,
        runtime_storage: &plan.runtime_storage,
        runtime_text: &plan.runtime_text,
        state_guards: &plan.state_guards,
        layouts: &plan.layouts,
        machine_program: &plan.machine_program,
        encoded_machine: &plan.encoded_machine,
        object: &plan.object,
        relocations: &plan.relocations,
    })
}

fn write_emission_plan(
    options: &CompileOptions,
    emission_plan: &omega_artifacts::EmissionPlan,
) -> Result<(), Vec<Diagnostic>> {
    let writer = ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_emission_plan(emission_plan)
        .map_err(|diagnostic| vec![diagnostic])
}

fn ensure_emission_ready(
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

fn write_output(options: &CompileOptions, emitted: EmittedProgram) -> Result<(), Vec<Diagnostic>> {
    let build_dir = options.build_dir();
    std::fs::create_dir_all(&build_dir).map_err(io_diagnostic)?;

    if can_emit_executable_image(emitted.target) {
        let image = emit_checked_executable_image(
            ExecutableImageInput {
                target: emitted.target,
                object: &emitted.object,
                relocations: &emitted.relocations,
                text_bytes: &emitted.text_bytes,
                data_bytes: &emitted.data_bytes,
            },
            emitted.planned_text_bytes,
        )
        .map_err(|diagnostic| vec![diagnostic])?;

        let output_path = build_dir.join(&image.file_name);
        std::fs::write(&output_path, &image.bytes).map_err(io_diagnostic)?;
        mark_executable_if_needed(&output_path).map_err(|diagnostic| vec![diagnostic])?;
        return Ok(());
    }

    let object_container = emit_omega_object_container(ObjectContainerInput {
        target: emitted.target,
        object: &emitted.object,
        relocations: &emitted.relocations,
        text_bytes: &emitted.text_bytes,
        data_bytes: &emitted.data_bytes,
    });
    let output_path = build_dir.join(&object_container.file_name);
    std::fs::write(&output_path, &object_container.bytes).map_err(io_diagnostic)?;
    Ok(())
}

fn io_diagnostic(error: std::io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::error(error.to_string())]
}

fn object_text_size(object: &omega_object::ObjectPlan) -> usize {
    object
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Text)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

#[cfg(unix)]
fn mark_executable_if_needed(path: &std::path::Path) -> Result<(), Diagnostic> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| Diagnostic::error(format!("failed to read {}: {error}", path.display())))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        Diagnostic::error(format!("failed to mark {} executable: {error}", path.display()))
    })
}

#[cfg(not(unix))]
fn mark_executable_if_needed(_path: &std::path::Path) -> Result<(), Diagnostic> {
    Ok(())
}
