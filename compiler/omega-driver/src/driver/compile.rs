use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::ast::item::Item;
use crate::diagnostics::Diagnostic;
use crate::driver::CompileOptions;
use crate::driver::artifacts::ArtifactWriter;
use crate::driver::trust::build_trust_report;
use crate::ir::lowering::lower_program;
use crate::lexer::{Lexer, Span};
use crate::native::control_flow::build_control_flow_plan;
use crate::native::emission::build_emission_plan;
use crate::native::plan::build_native_plan;
use crate::native::target::NativeTarget;
use crate::parser::parser::parse_file;
use crate::proof::checker::check_proof_plan;
use crate::proof::obligations::build_proof_plan;
use crate::semantic::effects::infer_effects;
use crate::semantic::validation::validate_program;
use crate::source::{Resolver, SourceFile};
use omega_graph::build_source_graph_report;
use omega_native::build_native_surface_report;
use omega_proof::build_proof_surface_report;
use omega_resolve::build_resolve_report;
use omega_types::build_type_surface_report;

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
    let mut phase_timings = Vec::new();
    let loaded_program = record_phase(&mut phase_timings, "sources", || {
        let loaded_program = load_program_sources(&options)?;
        debug_assert!(loaded_program.file_ranges_are_valid());
        artifacts
            .write_sources(&loaded_program)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(loaded_program)
    })?;

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
    let program = record_phase(&mut phase_timings, "driver ir lowering", || {
        lower_program(&loaded_program.items).map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "types/effects", || {
        let type_surface = build_type_surface_report(&loaded_program.items);
        let effect_plan = infer_effects(&program);
        artifacts
            .write_type_surface_and_effects(&type_surface, &effect_plan)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "driver ir", || {
        artifacts
            .write_ir(&program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "validation", || {
        validate_program(&program)?;
        artifacts
            .write_validation(&program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "graph", || {
        let source_graph = build_source_graph_report(&loaded_program.items);
        let control_flow =
            build_control_flow_plan(&program).map_err(|diagnostic| vec![diagnostic])?;
        artifacts
            .write_graphs(&source_graph, &control_flow)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "proof", || {
        let proof_surface = build_proof_surface_report(&loaded_program.items);
        let proof_plan = build_proof_plan(&program);
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
    record_phase(&mut phase_timings, "native plan", || {
        let native_surface = build_native_surface_report(&loaded_program.items);
        let native_plan = build_native_plan(
            &program,
            NativeTarget::from_omega_target_name(options.target_name.as_deref()),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        artifacts
            .write_native_report(&native_surface, &native_plan)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "emission plan", || {
        let native_plan = build_native_plan(
            &program,
            NativeTarget::from_omega_target_name(options.target_name.as_deref()),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
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
    let mut phase_timings = Vec::new();
    let loaded_program = record_phase(&mut phase_timings, "sources", || {
        let loaded_program = load_program_sources(&options)?;
        debug_assert!(loaded_program.file_ranges_are_valid());
        artifacts
            .write_sources(&loaded_program)
            .map_err(|diagnostic| vec![diagnostic])?;

        Ok(loaded_program)
    })?;

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
    let program = record_phase(&mut phase_timings, "driver ir lowering", || {
        lower_program(&loaded_program.items).map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "types/effects", || {
        let type_surface = build_type_surface_report(&loaded_program.items);
        let effect_plan = infer_effects(&program);
        artifacts
            .write_type_surface_and_effects(&type_surface, &effect_plan)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "driver ir", || {
        artifacts
            .write_ir(&program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "validation", || {
        validate_program(&program)?;
        artifacts
            .write_validation(&program)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "graph", || {
        let source_graph = build_source_graph_report(&loaded_program.items);
        let control_flow =
            build_control_flow_plan(&program).map_err(|diagnostic| vec![diagnostic])?;
        artifacts
            .write_graphs(&source_graph, &control_flow)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    record_phase(&mut phase_timings, "proof", || {
        let proof_surface = build_proof_surface_report(&loaded_program.items);
        let proof_plan = build_proof_plan(&program);
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
        let native_plan = build_native_plan(
            &program,
            NativeTarget::from_omega_target_name(options.target_name.as_deref()),
        )
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
    artifacts
        .write_timings(&phase_timings)
        .map_err(|diagnostic| vec![diagnostic])?;

    Err(vec![Diagnostic::error(format!(
        "native object emission is not implemented yet; artifacts {}; phases {}; planned {} host ABI binding(s), {} host call(s), {} data byte(s), {} selected instruction(s), {} instruction operand(s), {} machine code byte(s), {} encoded machine byte(s), {} relocation(s), {} data layout(s), {} machine layout(s), {} control-flow machine(s), {} object section(s), {} emission blocker(s), entry {}.{} as `{}`",
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
        native_plan.layouts.data_layouts.len(),
        native_plan.layouts.machine_layouts.len(),
        native_plan.control_flow.machines.len(),
        native_plan.object.sections.len(),
        emission_plan.blockers.len(),
        native_plan.entry_machine,
        native_plan.entry_state,
        native_plan.object.entry_symbol
    ))])
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
        output.push_str(&timing.microseconds.to_string());
        output.push_str("us");
    }

    output
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

fn load_program_sources(options: &CompileOptions) -> Result<LoadedProgram, Vec<Diagnostic>> {
    let mut resolver = Resolver::default();
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

    while let Some(path) = pending.pop() {
        let normalized = normalize_path(&path)?;

        if seen.contains(&normalized) {
            continue;
        }

        seen.push(normalized.clone());

        let file = resolver
            .load_root(&normalized)
            .map_err(|diagnostic| vec![diagnostic])?;
        let tokens = Lexer::new(&file.source).tokenize().map_err(|error| {
            vec![Diagnostic::error(format_source_span(
                file,
                error.span,
                &error.message,
            ))]
        })?;
        let ast_file = parse_file(&tokens).map_err(|error| {
            vec![Diagnostic::error(match error.span {
                Some(span) => format_source_span(file, span, &error.message),
                None => format!("{}: {}", file.path.display(), error.message),
            })]
        })?;
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
    path
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
        .nth(2)
        .expect("driver crate should live under compiler/omega-driver")
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
