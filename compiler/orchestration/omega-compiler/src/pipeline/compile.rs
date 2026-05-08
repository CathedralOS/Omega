use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::ast::item::Item;
use crate::lexer::{Lexer, Span};
use crate::parser::AstFile;
use crate::parser::parser::parse_file;
use crate::pipeline::CompileOptions;
use crate::pipeline::artifacts::ArtifactWriter;
use crate::pipeline::trust::build_trust_report;
use crate::source::{FileId, SourceFile};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPool;
use omega_effects::infer_effects;
use omega_graph::build_source_graph_report;
use omega_names::build_resolve_report;
use omega_native::build_native_surface_report;
use omega_native::control_flow::build_control_flow_plan_with_workers;
use omega_native::emission::build_emission_plan;
use omega_native::emitter::emit_native_output;
use omega_native::executable_finalization::{ExecutableFinalizationStatus, finalize_native_output};
use omega_native::plan::build_native_plan_with_workers;
use omega_native::target::NativeTarget;
use omega_proof::build_proof_surface_report;
use omega_proof::checker::check_proof_plan;
use omega_proof::obligations::build_proof_plan;
use omega_typed_program::lowering::lower_program;
use omega_types::build_type_surface_report;
use omega_validation::validate_program;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTiming {
    pub phase: String,
    pub microseconds: u128,
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
            .write_sources(&loaded_program)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(loaded_program)
    })?);

    debug_assert!(loaded_program.file_ranges_are_valid());
    record_phase(&mut phase_timings, "ast", || {
        artifacts
            .write_ast(&loaded_program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "resolve", || {
        let resolve_report = build_resolve_report(&loaded_program.items);
        artifacts
            .write_resolve_report(&resolve_report)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    let program = Arc::new(record_phase(
        &mut phase_timings,
        "typed program lowering",
        || lower_program(&loaded_program.items).map_err(|diagnostic| vec![diagnostic]),
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
    record_phase(&mut phase_timings, "graph", || {
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
            .map_err(|diagnostic| vec![diagnostic])
    })?;
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
    let native_plan = record_phase(&mut phase_timings, "native plan", || {
        let native_surface = build_native_surface_report(&loaded_program.items);
        let target = NativeTarget::from_omega_target_name(options.target_name.as_deref())
            .map_err(|diagnostic| vec![diagnostic])?;
        let native_plan =
            build_native_plan_with_workers(Arc::clone(&program), target, workers.handle())
                .map_err(|diagnostic| vec![diagnostic])?;
        artifacts
            .write_native_report(&native_surface, &native_plan)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(native_plan)
    })?;
    record_phase(&mut phase_timings, "emission plan", || {
        let emission_plan = build_emission_plan(&native_plan);
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
            .write_sources(&loaded_program)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(loaded_program)
    })?);

    debug_assert!(loaded_program.file_ranges_are_valid());
    record_phase(&mut phase_timings, "ast", || {
        artifacts
            .write_ast(&loaded_program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "resolve", || {
        let resolve_report = build_resolve_report(&loaded_program.items);
        artifacts
            .write_resolve_report(&resolve_report)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    let program = Arc::new(record_phase(
        &mut phase_timings,
        "typed program lowering",
        || lower_program(&loaded_program.items).map_err(|diagnostic| vec![diagnostic]),
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
    record_phase(&mut phase_timings, "graph", || {
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
            .map_err(|diagnostic| vec![diagnostic])
    })?;
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
    let native_plan = record_phase(&mut phase_timings, "native plan", || {
        let native_surface = build_native_surface_report(&loaded_program.items);
        let target = NativeTarget::from_omega_target_name(options.target_name.as_deref())
            .map_err(|diagnostic| vec![diagnostic])?;
        let native_plan =
            build_native_plan_with_workers(Arc::clone(&program), target, workers.handle())
                .map_err(|diagnostic| vec![diagnostic])?;
        artifacts
            .write_native_report(&native_surface, &native_plan)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(native_plan)
    })?;
    let emission_plan = record_phase(&mut phase_timings, "emission plan", || {
        let emission_plan = build_emission_plan(&native_plan);
        artifacts
            .write_emission_plan(&emission_plan)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(emission_plan)
    })?;
    if !emission_plan.blockers.is_empty() {
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
            let emitted_output =
                emit_native_output(&native_plan).map_err(|diagnostic| vec![diagnostic])?;
            let native_output_path = artifacts
                .write_emitted_native_output(&emitted_output)
                .map_err(|diagnostic| vec![diagnostic])?;
            Ok((native_output_path, emitted_output))
        })?;
    let executable_finalization = record_phase(&mut phase_timings, "finalize executable", || {
        let executable_finalization = finalize_native_output(
            &native_plan,
            &emitted_output,
            &native_output_path,
            artifacts.root(),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        artifacts
            .write_executable_finalization_report(&executable_finalization)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(executable_finalization)
    })?;
    let executable_path = executable_finalization.executable_path.clone();
    let executable_action = match executable_finalization.status {
        ExecutableFinalizationStatus::UsedExternalLinker => "linked",
        ExecutableFinalizationStatus::AlreadyExecutable => "finalized",
    };
    artifacts
        .write_timings(&phase_timings)
        .map_err(|diagnostic| vec![diagnostic])?;

    Ok(CompileOutput {
        artifacts_dir: artifacts.root().to_path_buf(),
        executable_path: executable_path.clone(),
        summary: format!(
            "emitted {}; {executable_action} {}; artifacts {}; phases {}; planned {} host ABI binding(s), {} host call(s), {} data byte(s), {} selected instruction(s), {} instruction operand(s), {} machine code byte(s), {} encoded machine byte(s), {} relocation(s), {} emission blocker(s), entry {}.{} as `{}`",
            native_output_path.display(),
            executable_path.display(),
            artifacts.root().display(),
            format_phase_timings(&phase_timings),
            native_plan.host_abi.bindings.len(),
            native_plan.host_calls.calls.len(),
            native_plan.data.bytes.len(),
            native_plan.instructions.instructions.len(),
            native_plan.instructions.operands.len(),
            native_plan.machine_code.byte_count,
            native_plan.machine_code.bytes.len(),
            native_plan.relocations.records.len(),
            emission_plan.blockers.len(),
            native_plan.entry_machine,
            native_plan.entry_state,
            native_plan.object.entry_symbol
        ),
        phase_timings,
    })
}

fn record_phase<T>(
    timings: &mut Vec<PhaseTiming>,
    phase: &str,
    action: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let started_at = Instant::now();
    let result = action();

    timings.push(PhaseTiming {
        phase: phase.to_owned(),
        microseconds: started_at.elapsed().as_micros(),
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
    pub(crate) items: Vec<Item>,
    pub(crate) files: Vec<LoadedFile>,
}

#[derive(Debug)]
pub(crate) struct LoadedFile {
    pub(crate) path: PathBuf,
    pub(crate) first_item: usize,
    pub(crate) item_count: usize,
}

impl LoadedProgram {
    fn file_ranges_are_valid(&self) -> bool {
        self.files.iter().all(|file| {
            !file.path.as_os_str().is_empty()
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

    let mut seen = Vec::<PathBuf>::new();
    let mut pending = Vec::new();
    let mut items = Vec::new();
    let mut loaded_files = Vec::new();
    let mut selected_target_found = options.target_name.is_none();

    if let Some(build_path) = build_policy_path(&options.root_path) {
        pending.push(build_path);
    }

    pending.push(options.root_path.clone());

    while !pending.is_empty() {
        let mut batch_paths = Vec::new();
        while let Some(path) = pending.pop() {
            let normalized = normalize_path(&path)?;

            if seen.contains(&normalized) {
                continue;
            }

            seen.push(normalized.clone());
            batch_paths.push(normalized);
        }

        let first_file_id = seen.len() - batch_paths.len();
        let files = Arc::new(load_source_batch(&workers, batch_paths, first_file_id)?);

        let ast_files = parse_source_batch(&workers, Arc::clone(&files))?;

        for (file, ast_file) in files.iter().zip(ast_files) {
            let first_item = items.len();
            let item_count = ast_file.items.len();

            for item in &ast_file.items {
                match item {
                    Item::Use(use_item) => {
                        pending.push(resolve_source_path(&root_dir, &use_item.path));
                    }
                    Item::Target(target) => {
                        let target_is_selected = options
                            .target_name
                            .as_ref()
                            .is_none_or(|target_name| target.name == *target_name);

                        if target_is_selected {
                            selected_target_found = true;
                        } else {
                            continue;
                        }

                        if let Some(host) = &target.host {
                            if is_bundled_omega_path(&host.provider) {
                                pending.push(resolve_source_path(&root_dir, &host.provider));
                            }
                        }

                        for trust_policy in &target.trust_policies {
                            if is_bundled_omega_path(&trust_policy.path) {
                                pending.push(resolve_source_path(&root_dir, &trust_policy.path));
                            }
                        }
                    }
                    _ => {}
                }
            }

            loaded_files.push(LoadedFile {
                path: file.path.clone(),
                first_item,
                item_count,
            });
            items.extend(ast_file.items);
        }
    }

    if !selected_target_found {
        return Err(vec![Diagnostic::error(format!(
            "target `{}` was not found in build policy",
            options
                .target_name
                .as_deref()
                .expect("missing selected target should have been detected")
        ))]);
    }

    Ok(LoadedProgram {
        items,
        files: loaded_files,
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
            source,
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
    let tokens = Lexer::new(&file.source).tokenize().map_err(|error| {
        vec![Diagnostic::error(format_source_span(
            file,
            error.span,
            &error.message,
        ))]
    })?;

    parse_file(&tokens).map_err(|error| {
        vec![Diagnostic::error(match error.span {
            Some(span) => format_source_span(file, span, &error.message),
            None => format!("{}: {}", file.path.display(), error.message),
        })]
    })
}

fn resolve_source_path(root_dir: &Path, source_path: &[String]) -> PathBuf {
    let mut segments = source_path.iter();
    let mut path = if is_bundled_omega_path(source_path) {
        segments.next();
        bundled_omega_root()
    } else {
        root_dir.to_path_buf()
    };

    for segment in segments {
        path.push(segment);
    }

    path.set_extension("omg");

    if path.exists() {
        return path;
    }

    path.set_extension("");
    path.join("mod.omg")
}

fn is_bundled_omega_path(path: &[String]) -> bool {
    path.first().is_some_and(|segment| segment == "omega")
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

fn build_policy_path(root_path: &Path) -> Option<PathBuf> {
    let build_path = root_path.parent()?.join("build.omg");

    build_path.exists().then_some(build_path)
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
