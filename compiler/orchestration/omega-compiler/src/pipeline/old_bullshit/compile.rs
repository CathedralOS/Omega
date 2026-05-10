use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::ast::identifier::IdentifierPath;
use crate::ast::item::Item;
use crate::lexer::{Lexer, Span};
use crate::parser::AstFile;
use crate::pipeline::CompileOptions;
use crate::pipeline::artifact_inputs::{ast_artifact, source_load_artifact};
use crate::pipeline::artifacts::ArtifactWriter;
use crate::pipeline::backend_report;
use crate::pipeline::trust::build_trust_report;
use crate::source::{FileId, SourceFile, SourceMap};
use omega_abstract_syntax_to_typed::{
    AstLoweringInput, lower_program_input_with_symbol_table_and_workers,
};
use omega_artifacts::{PhaseTiming, build_backend_surface_report, finalize_emitted_image_output};
use omega_backend_pipeline::build_backend_plan_from_control_flow_with_workers;
use omega_backend_plan::BackendPlan;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::allocations::snapshot as allocation_snapshot;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPool;
use omega_effects::infer_effects;
use omega_emission_planning::{EmissionPlanningInput, build_emission_plan};
use omega_graph::build_source_graph_report;
use omega_image_emission::{ExecutableImageInput, emit_checked_executable_image};
use omega_names::build_resolve_report;
use omega_proof::build_proof_surface_report;
use omega_proof::checker::check_proof_plan;
use omega_proof::obligations::build_proof_plan;
use omega_target::NativeTarget;
use omega_typed_to_control_flow::build_control_flow_plan_with_workers;
use omega_types::build_type_surface_report;
use omega_validation::validate_program;

fn emission_planning_input(backend_plan: &BackendPlan) -> EmissionPlanningInput<'_> {
    EmissionPlanningInput {
        target: backend_plan.target,
        entry_key: backend_plan.entry_key,
        host_abi: &backend_plan.host_abi,
        host_calls: &backend_plan.host_calls,
        state_calls: &backend_plan.state_calls,
        state_storage: &backend_plan.state_storage,
        state_values: &backend_plan.state_values,
        data: &backend_plan.data,
        instructions: &backend_plan.instructions,
        control_flow: &backend_plan.control_flow,
        runtime_flow: &backend_plan.runtime_flow,
        runtime_bodies: &backend_plan.runtime_bodies,
        runtime_branching_calls: &backend_plan.runtime_branching_calls,
        runtime_dispatch_loop: &backend_plan.runtime_dispatch_loop,
        runtime_storage: &backend_plan.runtime_storage,
        runtime_text: &backend_plan.runtime_text,
        state_guards: &backend_plan.state_guards,
        layouts: &backend_plan.layouts,
        machine_code: &backend_plan.machine_code,
        encoded_machine: &backend_plan.encoded_machine,
        object: &backend_plan.object,
        relocations: &backend_plan.relocations,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    pub summary: String,
    pub artifacts_dir: PathBuf,
    pub executable_path: PathBuf,
    pub phase_timings: Vec<PhaseTiming>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutput {
    pub summary: String,
    pub artifacts_dir: PathBuf,
    pub phase_timings: Vec<PhaseTiming>,
}

pub fn check(options: CompileOptions) -> Result<CheckOutput, Vec<Diagnostic>> {
    let build_dir = options.build_dir();
    let artifacts = ArtifactWriter::new(&build_dir).map_err(|diagnostic| vec![diagnostic])?;
    let workers = WorkerPool::with_available_parallelism();
    let mut phase_timings = Vec::new();
    let loaded_program = Arc::new(record_phase(&mut phase_timings, "sources", || {
        let loaded_program = load_program_sources(&options, &workers)?;
        debug_assert!(loaded_program.file_ranges_are_valid());
        artifacts
            .write_sources(&source_load_artifact(&loaded_program))
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(loaded_program)
    })?);

    debug_assert!(loaded_program.file_ranges_are_valid());
    record_phase(&mut phase_timings, "ast", || {
        artifacts
            .write_ast(&ast_artifact(&loaded_program))
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    let resolve_report = Arc::new(record_phase(&mut phase_timings, "resolve", || {
        let resolve_report =
            build_resolve_report(&loaded_program.items, Arc::clone(&loaded_program.sources));
        artifacts
            .write_resolve_report(&resolve_report)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(resolve_report)
    })?);
    let program = Arc::new(record_phase(
        &mut phase_timings,
        "typed program lowering",
        || {
            let lowering_input = AstLoweringInput::new(Arc::clone(&loaded_program.items))
                .with_syntax_tables(Arc::clone(&loaded_program.syntax_tables))
                .with_sources(Arc::clone(&loaded_program.sources));

            lower_program_input_with_symbol_table_and_workers(
                lowering_input,
                resolve_report.symbols.clone(),
                workers.handle(),
            )
            .map_err(|diagnostic| vec![diagnostic])
        },
    )?);
    record_phase(&mut phase_timings, "types/effects", || {
        let loaded_program_for_types = Arc::clone(&loaded_program);
        let program_for_effects = Arc::clone(&program);
        let (type_surface, effect_plan) = workers.handle().join2(
            move || build_type_surface_report(&loaded_program_for_types.items),
            move || infer_effects(&program_for_effects),
        );
        artifacts
            .write_type_surface_and_effects(&type_surface, &effect_plan)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "typed program", || {
        artifacts
            .write_typed_program(&program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "validation", || {
        validate_program(&program)?;
        artifacts
            .write_validation(&program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    let control_flow = Arc::new(record_phase(&mut phase_timings, "graph", || {
        let loaded_program_for_graph = Arc::clone(&loaded_program);
        let program_for_control_flow = Arc::clone(&program);
        let workers_for_control_flow = workers.handle();
        let (source_graph, control_flow) = workers.handle().join2(
            move || build_source_graph_report(&loaded_program_for_graph.items),
            move || {
                build_control_flow_plan_with_workers(
                    program_for_control_flow,
                    workers_for_control_flow,
                )
            },
        );
        let control_flow = control_flow.map_err(|diagnostic| vec![diagnostic])?;
        artifacts
            .write_graphs(&source_graph, &control_flow)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(control_flow)
    })?);
    record_phase(&mut phase_timings, "proof", || {
        let loaded_program_for_proof = Arc::clone(&loaded_program);
        let program_for_proof = Arc::clone(&program);
        let (proof_surface, proof_plan) = workers.handle().join2(
            move || build_proof_surface_report(&loaded_program_for_proof.items),
            move || build_proof_plan(&program_for_proof),
        );
        artifacts
            .write_proof_report(&proof_surface, &proof_plan)
            .map_err(|diagnostic| vec![diagnostic])?;
        check_proof_plan(&proof_plan)
    })?;
    record_phase(&mut phase_timings, "trust", || {
        let trust_report =
            build_trust_report(&loaded_program.items, options.target_name.as_deref());
        artifacts
            .write_trust_report(&trust_report)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    let backend_plan = record_phase(&mut phase_timings, "backend plan", || {
        let target = NativeTarget::from_omega_target_name(options.target_name.as_deref())
            .map_err(|diagnostic| vec![diagnostic])?;
        let backend_plan = build_backend_plan_from_control_flow_with_workers(
            Arc::clone(&program),
            target,
            Arc::clone(&control_flow),
            workers.handle(),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let backend_surface = build_backend_surface_report(&program);
        backend_report::write_backend_report(&artifacts, &backend_surface, &backend_plan)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(backend_plan)
    })?;
    record_phase(&mut phase_timings, "emission plan", || {
        let emission_plan = build_emission_plan(&emission_planning_input(&backend_plan));
        artifacts
            .write_emission_plan(&emission_plan)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    artifacts
        .write_timings(&phase_timings)
        .map_err(|diagnostic| vec![diagnostic])?;

    Ok(CheckOutput {
        artifacts_dir: artifacts.root().to_path_buf(),
        summary: format!(
            "checked {}; artifacts {}; phases {}",
            options.root_path.display(),
            artifacts.root().display(),
            format_phase_timings(&phase_timings)
        ),
        phase_timings,
    })
}

pub fn compile(options: CompileOptions) -> Result<CompileOutput, Vec<Diagnostic>> {
    let build_dir = options.build_dir();
    let artifacts = ArtifactWriter::new(&build_dir).map_err(|diagnostic| vec![diagnostic])?;
    let workers = WorkerPool::with_available_parallelism();
    let mut phase_timings = Vec::new();
    let loaded_program = Arc::new(record_phase(&mut phase_timings, "sources", || {
        let loaded_program = load_program_sources(&options, &workers)?;
        debug_assert!(loaded_program.file_ranges_are_valid());
        artifacts
            .write_sources(&source_load_artifact(&loaded_program))
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(loaded_program)
    })?);

    debug_assert!(loaded_program.file_ranges_are_valid());
    record_phase(&mut phase_timings, "ast", || {
        artifacts
            .write_ast(&ast_artifact(&loaded_program))
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    let resolve_report = Arc::new(record_phase(&mut phase_timings, "resolve", || {
        let resolve_report =
            build_resolve_report(&loaded_program.items, Arc::clone(&loaded_program.sources));
        artifacts
            .write_resolve_report(&resolve_report)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(resolve_report)
    })?);
    let program = Arc::new(record_phase(
        &mut phase_timings,
        "typed program lowering",
        || {
            let lowering_input = AstLoweringInput::new(Arc::clone(&loaded_program.items))
                .with_syntax_tables(Arc::clone(&loaded_program.syntax_tables))
                .with_sources(Arc::clone(&loaded_program.sources));

            lower_program_input_with_symbol_table_and_workers(
                lowering_input,
                resolve_report.symbols.clone(),
                workers.handle(),
            )
            .map_err(|diagnostic| vec![diagnostic])
        },
    )?);
    record_phase(&mut phase_timings, "types/effects", || {
        let loaded_program_for_types = Arc::clone(&loaded_program);
        let program_for_effects = Arc::clone(&program);
        let (type_surface, effect_plan) = workers.handle().join2(
            move || build_type_surface_report(&loaded_program_for_types.items),
            move || infer_effects(&program_for_effects),
        );
        artifacts
            .write_type_surface_and_effects(&type_surface, &effect_plan)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "typed program", || {
        artifacts
            .write_typed_program(&program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "validation", || {
        validate_program(&program)?;
        artifacts
            .write_validation(&program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    let control_flow = Arc::new(record_phase(&mut phase_timings, "graph", || {
        let loaded_program_for_graph = Arc::clone(&loaded_program);
        let program_for_control_flow = Arc::clone(&program);
        let workers_for_control_flow = workers.handle();
        let (source_graph, control_flow) = workers.handle().join2(
            move || build_source_graph_report(&loaded_program_for_graph.items),
            move || {
                build_control_flow_plan_with_workers(
                    program_for_control_flow,
                    workers_for_control_flow,
                )
            },
        );
        let control_flow = control_flow.map_err(|diagnostic| vec![diagnostic])?;
        artifacts
            .write_graphs(&source_graph, &control_flow)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(control_flow)
    })?);
    record_phase(&mut phase_timings, "proof", || {
        let loaded_program_for_proof = Arc::clone(&loaded_program);
        let program_for_proof = Arc::clone(&program);
        let (proof_surface, proof_plan) = workers.handle().join2(
            move || build_proof_surface_report(&loaded_program_for_proof.items),
            move || build_proof_plan(&program_for_proof),
        );
        artifacts
            .write_proof_report(&proof_surface, &proof_plan)
            .map_err(|diagnostic| vec![diagnostic])?;
        check_proof_plan(&proof_plan)
    })?;
    record_phase(&mut phase_timings, "trust", || {
        let trust_report =
            build_trust_report(&loaded_program.items, options.target_name.as_deref());
        artifacts
            .write_trust_report(&trust_report)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    let backend_plan = record_phase(&mut phase_timings, "backend plan", || {
        let target = NativeTarget::from_omega_target_name(options.target_name.as_deref())
            .map_err(|diagnostic| vec![diagnostic])?;
        let backend_plan = build_backend_plan_from_control_flow_with_workers(
            Arc::clone(&program),
            target,
            Arc::clone(&control_flow),
            workers.handle(),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let backend_surface = build_backend_surface_report(&program);
        backend_report::write_backend_report(&artifacts, &backend_surface, &backend_plan)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(backend_plan)
    })?;
    let emission_plan = record_phase(&mut phase_timings, "emission plan", || {
        let emission_plan = build_emission_plan(&emission_planning_input(&backend_plan));
        artifacts
            .write_emission_plan(&emission_plan)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(emission_plan)
    })?;
    if !emission_plan.blockers.is_empty() {
        artifacts
            .remove_stale_native_output()
            .map_err(|diagnostic| vec![diagnostic])?;
        return Err(emission_plan
            .blockers
            .iter()
            .map(|(_, blocker)| {
                Diagnostic::error(format!(
                    "cannot emit native binary; {}: {}",
                    blocker.stage, blocker.reason
                ))
            })
            .collect());
    }
    let (native_output_path, emitted_output) =
        record_phase(&mut phase_timings, "emit native output", || {
            let emitted_output = emit_checked_executable_image(
                ExecutableImageInput {
                    target: backend_plan.target,
                    object: &backend_plan.object,
                    relocations: &backend_plan.relocations,
                    text_bytes: backend_plan.encoded_machine.bytes.storage_slice(),
                    data_bytes: backend_plan.data.bytes.storage_slice(),
                },
                backend_plan.machine_code.byte_count,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            let native_output_path = artifacts
                .write_emitted_native_output(&emitted_output)
                .map_err(|diagnostic| vec![diagnostic])?;
            Ok((native_output_path, emitted_output))
        })?;
    let executable_finalization = record_phase(&mut phase_timings, "finalize executable", || {
        let executable_finalization = finalize_emitted_image_output(
            backend_plan.target,
            &emitted_output,
            &native_output_path,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        artifacts
            .write_executable_finalization_report(&executable_finalization)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(executable_finalization)
    })?;
    let executable_path = executable_finalization.executable_path.clone();
    artifacts
        .write_timings(&phase_timings)
        .map_err(|diagnostic| vec![diagnostic])?;

    Ok(CompileOutput {
        artifacts_dir: artifacts.root().to_path_buf(),
        executable_path: executable_path.clone(),
        summary: format!(
            "emitted {}; finalized {}; artifacts {}; phases {}; planned {} host ABI binding(s), {} host call(s), {} data byte(s), {} selected instruction(s), {} instruction operand(s), {} machine code byte(s), {} encoded machine byte(s), {} relocation(s), {} emission blocker(s), entry {}.{} as `{}`",
            native_output_path.display(),
            executable_path.display(),
            artifacts.root().display(),
            format_phase_timings(&phase_timings),
            backend_plan.host_abi.bindings.len(),
            backend_plan.host_calls.calls.len(),
            backend_plan.data.bytes.len(),
            backend_plan.instructions.instructions.len(),
            backend_plan.instructions.operands.len(),
            backend_plan.machine_code.byte_count,
            backend_plan.encoded_machine.bytes.len(),
            backend_plan.relocations.records.len(),
            emission_plan.blockers.len(),
            backend_plan.entry_machine_name(),
            backend_plan.entry_state_name(),
            backend_plan.object.entry_symbol
        ),
        phase_timings,
    })
}

fn record_phase<T>(
    timings: &mut Vec<PhaseTiming>,
    phase: &str,
    action: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let allocation_before = allocation_snapshot();
    let started_at = Instant::now();
    let result = action();
    let microseconds = started_at.elapsed().as_micros();
    let allocation_after = allocation_snapshot();

    timings.push(PhaseTiming {
        phase: phase.to_owned(),
        microseconds,
        allocations: allocation_after.delta_since(allocation_before),
    });

    result
}

fn format_phase_timings(timings: &[PhaseTiming]) -> String {
    let mut output = String::new();

    for (index, timing) in timings.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }

        output.push_str(&timing.phase);
        output.push('=');
        output.push_str(&format_compact_duration(timing.microseconds));
    }

    output
}

fn format_compact_duration(microseconds: u128) -> String {
    if microseconds >= 1_000_000 {
        format!("{:.2}s", microseconds as f64 / 1_000_000.0)
    } else if microseconds >= 1_000 {
        format!("{:.2}ms", microseconds as f64 / 1_000.0)
    } else {
        format!("{microseconds}us")
    }
}

#[derive(Debug)]
pub(crate) struct LoadedProgram {
    pub(crate) items: Arc<Vec<Item>>,
    pub(crate) files: Vec<LoadedFile>,
    pub(crate) sources: Arc<SourceMap>,
    pub(crate) syntax_tables: Arc<Vec<crate::ast::tables::AstTables>>,
}

#[derive(Debug)]
pub(crate) struct LoadedFile {
    pub(crate) file_id: FileId,
    pub(crate) path: PathBuf,
    pub(crate) first_item: usize,
    pub(crate) item_count: usize,
    pub(crate) syntax_table_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct DiscoveredSource {
    path: PathBuf,
}

#[derive(Debug, Default)]
struct SourceImportQueue {
    seen: HashSet<PathBuf>,
    discovered: Arena<DiscoveredSource>,
    frontier_cursor: usize,
}

impl SourceImportQueue {
    fn new() -> Self {
        Self::default()
    }

    fn enqueue(&mut self, path: PathBuf) -> bool {
        if !self.seen.insert(path.clone()) {
            return false;
        }

        self.discovered.append(DiscoveredSource { path });
        true
    }

    fn next_frontier(&mut self) -> Option<HandleSpan<DiscoveredSource>> {
        let total_discovered = self.seen.len();
        if self.frontier_cursor >= total_discovered {
            return None;
        }

        let new_count = total_discovered - self.frontier_cursor;
        let start_index = self
            .frontier_cursor
            .checked_add(1)
            .and_then(|index| u32::try_from(index).ok())
            .expect("frontier start index overflow");
        let count = u32::try_from(new_count).expect("frontier count overflow");
        self.frontier_cursor = total_discovered;

        Some(HandleSpan::from_parts(
            omega_core::arena::Handle::from_arena_index(start_index),
            count,
        ))
    }

    fn paths(&self, frontier: HandleSpan<DiscoveredSource>) -> Vec<PathBuf> {
        self.discovered
            .span_or_empty(frontier)
            .iter()
            .map(|source| source.path.clone())
            .collect()
    }
}

impl LoadedProgram {
    fn file_ranges_are_valid(&self) -> bool {
        self.files.iter().all(|file| {
            !file.path.as_os_str().is_empty()
                && self.sources.get(file.file_id).is_some()
                && file.first_item <= self.items.len()
                && file.first_item + file.item_count <= self.items.len()
        })
    }
}

fn load_program_sources(
    options: &CompileOptions,
    workers: &WorkerPool,
) -> Result<LoadedProgram, Vec<Diagnostic>> {
    let root_dir = options
        .root_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut queue = SourceImportQueue::new();
    let mut items = Vec::new();
    let mut loaded_files = Vec::new();
    let mut source_files = Vec::new();
    let mut syntax_tables = Vec::new();
    let mut selected_target_found = options.target_name.is_none();
    let root_path = normalize_path(&options.root_path)?;
    let _ = queue.enqueue(root_path);

    while let Some(frontier) = queue.next_frontier() {
        let batch_paths = queue.paths(frontier);
        let first_file_id = source_files.len();
        let files = Arc::new(load_source_batch(workers, batch_paths, first_file_id)?);

        let ast_files = parse_source_batch(&workers, Arc::clone(&files))?;
        let files = Arc::try_unwrap(files).unwrap_or_else(|files| files.as_ref().clone());

        for (file, ast_file) in files.into_iter().zip(ast_files) {
            let first_item = items.len();
            let item_count = ast_file.items.len();

            selected_target_found |= enqueue_imports_from_ast_items(
                &mut queue,
                &root_dir,
                &ast_file.items,
                options.target_name.as_deref(),
            )?;

            loaded_files.push(LoadedFile {
                file_id: file.id,
                path: file.path.clone(),
                first_item,
                item_count,
                syntax_table_index: syntax_tables.len(),
            });
            syntax_tables.push(ast_file.tables);
            items.extend(ast_file.items);
            source_files.push(file);
        }
    }

    if !selected_target_found {
        return Err(vec![Diagnostic::error(format!(
            "target `{}` was not found in discovered source frontier",
            options
                .target_name
                .as_deref()
                .expect("missing selected target should have been detected")
        ))]);
    }

    Ok(LoadedProgram {
        items: Arc::new(items),
        files: loaded_files,
        sources: Arc::new(SourceMap::from_files(source_files)),
        syntax_tables: Arc::new(syntax_tables),
    })
}

fn load_source_batch(
    workers: &WorkerPool,
    paths: Vec<PathBuf>,
    first_file_id: usize,
) -> Result<Vec<SourceFile>, Vec<Diagnostic>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    let paths = Arc::new(paths);
    let loaded = workers.map_ordered(paths.len(), move |index| {
        let path = paths
            .get(index)
            .cloned()
            .expect("source worker index should be in range");
        let source = std::fs::read_to_string(&path).map_err(|error| {
            Diagnostic::error(format!("failed to read {}: {error}", path.display()))
        });

        (index, path, source)
    });

    let mut files = Vec::with_capacity(loaded.len());
    for (index, path, source) in loaded {
        let source = source.map_err(|diagnostic| vec![diagnostic])?;
        files.push(SourceFile {
            id: FileId(first_file_id + index),
            path,
            source: Arc::from(source),
        });
    }

    Ok(files)
}

fn parse_source_batch(
    workers: &WorkerPool,
    files: Arc<Vec<SourceFile>>,
) -> Result<Vec<AstFile>, Vec<Diagnostic>> {
    if files.is_empty() {
        return Ok(Vec::new());
    }

    let parsed = workers.map_ordered(files.len(), move |index| {
        let file = files
            .get(index)
            .expect("parser worker index should be in range");

        parse_source_file(file)
    });

    let mut ast_files = Vec::with_capacity(parsed.len());
    for ast_file in parsed {
        ast_files.push(ast_file?);
    }

    Ok(ast_files)
}

fn parse_source_file(file: &SourceFile) -> Result<AstFile, Vec<Diagnostic>> {
    let tokens = Lexer::new(file.source.as_ref())
        .tokenize()
        .map_err(|error| {
            vec![Diagnostic::error(format_source_span(
                file,
                error.span,
                &error.message,
            ))]
        })?;

    crate::parser::parse_file_with_source(file.id, Arc::clone(&file.source), &tokens).map_err(
        |error| {
            vec![Diagnostic::error(match error.span {
                Some(span) => format_source_span(file, span, &error.message),
                None => format!("{}: {}", file.path.display(), error.message),
            })]
        },
    )
}

fn enqueue_imports_from_ast_items(
    queue: &mut SourceImportQueue,
    root_dir: &Path,
    items: &[Item],
    selected_target_name: Option<&str>,
) -> Result<bool, Vec<Diagnostic>> {
    let mut selected_target_found = selected_target_name.is_none();

    for item in items {
        match item {
            Item::Use(use_item) => {
                let path = normalize_path(&resolve_source_path(root_dir, &use_item.path))?;
                let _ = queue.enqueue(path);
            }
            Item::Target(target) => {
                let target_is_selected = selected_target_name
                    .is_none_or(|target_name| target.name.as_str() == target_name);

                if target_is_selected {
                    selected_target_found = true;
                } else {
                    continue;
                }

                if let Some(host) = &target.host {
                    if is_bundled_omega_path(&host.provider) {
                        let path =
                            normalize_path(&resolve_source_path(root_dir, &host.provider))?;
                        let _ = queue.enqueue(path);
                    }
                }

                for trust_policy in &target.trust_policies {
                    if is_bundled_omega_path(&trust_policy.path) {
                        let path =
                            normalize_path(&resolve_source_path(root_dir, &trust_policy.path))?;
                        let _ = queue.enqueue(path);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(selected_target_found)
}

fn resolve_source_path(root_dir: &Path, source_path: &IdentifierPath) -> PathBuf {
    let mut segments = source_path.iter();
    let mut path = if is_bundled_omega_path(source_path) {
        segments.next();
        bundled_omega_root()
    } else {
        root_dir.to_path_buf()
    };

    for segment in segments {
        path.push(segment.as_str());
    }

    path.set_extension("omg");

    if path.exists() {
        return path;
    }

    path.set_extension("");
    path.join("mod.omg")
}

fn is_bundled_omega_path(path: &IdentifierPath) -> bool {
    path.first()
        .is_some_and(|segment| segment.as_str() == "omega")
}

fn bundled_omega_root() -> PathBuf {
    if let Some(path) = std::env::var_os("OMEGA_LIBRARY_ROOT") {
        return PathBuf::from(path);
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .join("omega")
}

fn normalize_path(path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    path.canonicalize().map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to resolve {}: {error}",
            path.display()
        ))]
    })
}

fn format_source_span(file: &SourceFile, span: Span, message: &str) -> String {
    let start = file.position_at(span.start);
    let end = file.position_at(span.end);

    format!(
        "{}:{}:{}-{}:{}: {}",
        file.path.display(),
        start.line,
        start.column,
        end.line,
        end.column,
        message
    )
}
