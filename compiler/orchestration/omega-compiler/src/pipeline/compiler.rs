use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::compile_report::CompileReport;
use crate::pipeline::frontend::{
    discover_imports, extend_source_storage, lex_sources, load_sources, parse_sources,
};
use crate::pipeline::source::{ImportQueue, SourceStorage};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPool;
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
        imports.seed(self.options.root_path.clone());
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

        let syntax = assemble_syntax(&source_storage)?;
        let resolved = resolve_program(syntax)?;
        let typed = typecheck_program(resolved)?;
        let validated = validate_program(typed)?;
        let planned = plan_backend(validated, self.options.target_name.as_deref(), workers.handle())?;
        let emitted = emit_backend(planned)?;

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

struct AssembledSyntax {
    syntax_trees: SyntaxTrees,
}

struct ValidatedProgram {
    program: TypedProgram,
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
    let mut items = Vec::new();

    for (_, file) in _sources.files.iter() {
        items.extend(file.syntax_trees.items.clone());
    }

    Ok(AssembledSyntax {
        syntax_trees: SyntaxTrees::from_items(Default::default(), items),
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

fn validate_program(typed: TypedProgram) -> Result<ValidatedProgram, Vec<Diagnostic>> {
    omega_validation::validate_program(&typed)?;
    Ok(ValidatedProgram { program: typed })
}

fn plan_backend(
    validated: ValidatedProgram,
    target_name: Option<&str>,
    workers: omega_core::parallel::WorkerPoolHandle,
) -> Result<omega_backend_plan::BackendPlan, Vec<Diagnostic>> {
    let target =
        NativeTarget::from_omega_target_name(target_name).map_err(|diagnostic| vec![diagnostic])?;
    let program = std::sync::Arc::new(validated.program);
    let state_graph = omega_typed_trees_to_state_graph::build_state_graph_with_workers(
        std::sync::Arc::clone(&program),
        workers.clone(),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let control_flow =
        omega_state_graph_to_control_flow::build_control_flow_plan(&state_graph)
            .map_err(|diagnostic| vec![diagnostic])?;

    omega_backend_pipeline::build_backend_plan_from_control_flow_with_workers(
        program,
        target,
        std::sync::Arc::new(control_flow),
        workers,
    )
    .map_err(|diagnostic| vec![diagnostic])
}

fn emit_backend(plan: omega_backend_plan::BackendPlan) -> Result<EmittedProgram, Vec<Diagnostic>> {
    Ok(EmittedProgram {
        target: plan.target,
        planned_text_bytes: object_text_size(&plan.object),
        object: plan.object,
        relocations: plan.relocations,
        text_bytes: plan.encoded_machine.bytes.storage_slice().to_vec(),
        data_bytes: plan.data.bytes.storage_slice().to_vec(),
    })
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
