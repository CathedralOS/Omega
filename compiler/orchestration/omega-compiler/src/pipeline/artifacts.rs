use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::stages::AssembledSyntax;
use omega_artifacts::{ArtifactWriter, PhaseTiming};
use omega_backend_report::{BackendReportInput, BackendReportPhaseTiming, backend_report_text};
use omega_core::diagnostics::Diagnostic;
use std::path::Path;

pub(super) fn write_pipeline_index(options: &CompileOptions) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "00_pipeline.html",
        &omega_visualizations::pipeline_index_html(),
    )
}

pub(super) fn write_pipeline_shell(options: &CompileOptions) -> Result<(), Vec<Diagnostic>> {
    let build_dir = options.build_dir();
    let page_specs = [
        ("00", "Timings", "timings", "00_timings.html"),
        ("02", "Syntax", "syntax", "02_syntax_trees.html"),
        ("03", "Symbols", "symbols", "03_symbol_resolved_trees.html"),
        ("04", "Typed", "typed", "04_typed_trees.html"),
        ("05", "Checked", "checked", "05_checked_trees.html"),
        (
            "cap",
            "Capabilities",
            "capabilities",
            "05_capability_manifest.html",
        ),
        ("06", "State Graph", "state-graph", "06_state_graph.html"),
        ("07", "Control Flow", "control-flow", "07_control_flow.html"),
        (
            "08",
            "Abstract Operations",
            "abstract-operations",
            "08_abstract_operations.html",
        ),
        (
            "09",
            "Target Operations",
            "target-operations",
            "09_target_operations.html",
        ),
        (
            "10",
            "Assigned Target Operations",
            "assigned-target-operations",
            "10_assigned_target_operations.html",
        ),
        (
            "11",
            "Machine Instructions",
            "machine-instructions",
            "11_machine_instructions.html",
        ),
        ("12", "Emission", "emission", "12_emission.html"),
    ];
    let mut page_html = Vec::new();
    let mut present_page_specs = Vec::new();
    for page_spec in page_specs {
        let (_, _, _, file_name) = page_spec;
        let path = build_dir.join(file_name);
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        present_page_specs.push(page_spec);
        page_html.push(contents);
    }
    if present_page_specs.is_empty() {
        return write_pipeline_index(options);
    }

    let pages = present_page_specs
        .iter()
        .zip(page_html.iter())
        .map(
            |((number, label, id, _), html)| omega_visualizations::PipelineEmbeddedPage {
                number,
                label,
                id,
                html,
            },
        )
        .collect::<Vec<_>>();

    write_phase_diagram(
        options,
        "00_pipeline.html",
        &omega_visualizations::pipeline_shell_html(&pages),
    )
}

pub(super) fn write_syntax_snapshot(
    options: &CompileOptions,
    syntax: &AssembledSyntax,
) -> Result<(), Vec<Diagnostic>> {
    let files = syntax
        .files
        .iter()
        .map(|file| omega_visualizations::SyntaxSourceFile {
            path: display_path(&file.path),
            root_items: file.root_items.clone(),
        })
        .collect::<Vec<_>>();
    write_phase_diagram(
        options,
        "02_syntax_trees.html",
        &omega_visualizations::syntax_trees_with_files_html(&syntax.syntax_trees, &files),
    )?;
    write_phase_json(
        options,
        "02_syntax_trees.json",
        &syntax
            .syntax_trees
            .snapshot_json_pretty()
            .map_err(json_diagnostic)?,
    )
}

fn display_path(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    std::env::current_dir()
        .ok()
        .and_then(|current_dir| {
            canonical
                .strip_prefix(current_dir)
                .ok()
                .map(Path::to_path_buf)
        })
        .unwrap_or(canonical)
        .display()
        .to_string()
}

pub(super) fn write_resolved_snapshot(
    options: &CompileOptions,
    resolved: &omega_symbol_resolved_trees::SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "03_symbol_resolved_trees.html",
        &omega_visualizations::symbol_resolved_trees_html(resolved),
    )?;
    write_phase_json(
        options,
        "03_symbol_resolved_trees.json",
        &resolved.snapshot_json_pretty().map_err(json_diagnostic)?,
    )
}

pub(super) fn write_typed_snapshot(
    options: &CompileOptions,
    typed: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "04_typed_trees.html",
        &omega_visualizations::typed_trees_html(typed),
    )?;
    write_phase_json(
        options,
        "04_typed_trees.json",
        &typed.snapshot_json_pretty().map_err(json_diagnostic)?,
    )
}

pub(super) fn write_checked_snapshot(
    options: &CompileOptions,
    checked: &omega_checked_trees::CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "05_checked_trees.html",
        &omega_visualizations::checked_trees_html(checked),
    )?;
    write_phase_diagram(
        options,
        "05_capability_manifest.html",
        &omega_visualizations::capability_manifest_html(checked),
    )?;
    write_phase_json(
        options,
        "05_capability_manifest.json",
        &omega_visualizations::capability_manifest_json(checked),
    )
}

pub(super) fn write_state_graph_snapshot(
    options: &CompileOptions,
    state_graph: &omega_state_graph::StateGraph,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "06_state_graph.html",
        &omega_visualizations::state_graph_html(state_graph),
    )
}

pub(super) fn write_control_flow_snapshot(
    options: &CompileOptions,
    control_flow: &omega_control_flow::ControlFlowPlan,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "07_control_flow.html",
        &omega_visualizations::control_flow_html(control_flow),
    )
}

pub(super) fn write_backend_report(
    options: &CompileOptions,
    backend_surface: &omega_artifacts::BackendSurfaceReport,
    plan: &omega_backend_plan::BackendPlan,
) -> Result<(), Vec<Diagnostic>> {
    let phase_timings = plan
        .phase_timings
        .iter()
        .map(|(_, timing)| BackendReportPhaseTiming {
            phase: timing.phase.to_owned(),
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
            abstract_operations: &plan.abstract_operations,
            target_operations: &plan.target_operations,
            assigned_target_operations: &plan.assigned_target_operations,
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
            machine_instructions: &plan.machine_instructions,
            encoded_machine: &plan.encoded_machine,
            object: &plan.object,
            relocations: &plan.relocations,
        },
    );

    write_phase_diagram(
        options,
        "08_abstract_operations.html",
        &omega_visualizations::abstract_operations_html(
            &plan.abstract_operations,
            &plan.control_flow,
        ),
    )?;
    write_phase_diagram(
        options,
        "09_target_operations.html",
        &omega_visualizations::target_operations_html(&plan.target_operations, &plan.control_flow),
    )?;
    write_phase_diagram(
        options,
        "10_assigned_target_operations.html",
        &omega_visualizations::assigned_target_operations_html(
            &plan.assigned_target_operations,
            &plan.control_flow,
        ),
    )?;
    write_phase_diagram(
        options,
        "11_machine_instructions.html",
        &omega_visualizations::machine_instructions_html(
            &plan.machine_instructions,
            &plan.assigned_target_operations,
            &plan.control_flow,
        ),
    )?;
    write_phase_diagram(
        options,
        "backend_report.html",
        &omega_visualizations::text_report_html("backend_report", &report),
    )
}

pub(super) fn write_emission_plan(
    options: &CompileOptions,
    plan: &omega_backend_plan::BackendPlan,
    emission_plan: &omega_artifacts::EmissionPlan,
    output_path: Option<&Path>,
) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_emission_plan(emission_plan)
        .map_err(|diagnostic| vec![diagnostic])?;
    let disassembly = output_path
        .and_then(|output_path| load_native_disassembly(plan.target, output_path).ok())
        .flatten();
    write_phase_diagram(
        options,
        "12_emission.html",
        &omega_visualizations::emission_html(
            &plan.encoded_machine,
            &plan.machine_instructions,
            &plan.assigned_target_operations,
            &plan.control_flow,
            &plan.object,
            &plan.relocations,
            disassembly.as_deref(),
        ),
    )
}

fn load_native_disassembly(
    target: omega_target::NativeTarget,
    output_path: &Path,
) -> Result<Option<String>, Diagnostic> {
    match target.object_format {
        omega_target::ObjectFormat::MachO => {
            run_disassembler("otool", &["-tvV"], output_path).map(Some)
        }
        omega_target::ObjectFormat::Elf | omega_target::ObjectFormat::Coff => {
            for tool in ["llvm-objdump", "objdump"] {
                if let Ok(output) = run_disassembler(tool, &["-d"], output_path) {
                    return Ok(Some(output));
                }
            }
            Ok(None)
        }
    }
}

fn run_disassembler(tool: &str, args: &[&str], output_path: &Path) -> Result<String, Diagnostic> {
    let output = std::process::Command::new(tool)
        .args(args)
        .arg(output_path)
        .output()
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to run disassembler `{tool}` on {}: {error}",
                output_path.display()
            ))
        })?;
    if !output.status.success() {
        return Err(Diagnostic::error(format!(
            "disassembler `{tool}` failed on {} with status {}",
            output_path.display(),
            output.status
        )));
    }
    String::from_utf8(output.stdout).map_err(|error| {
        Diagnostic::error(format!(
            "disassembler `{tool}` produced non-utf8 output for {}: {error}",
            output_path.display()
        ))
    })
}

pub(super) fn write_timings(
    options: &CompileOptions,
    timings: &[PhaseTiming],
) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_timings(timings)
        .map_err(|diagnostic| vec![diagnostic])
}

pub(super) fn remove_stale_phase_diagrams(options: &CompileOptions) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .remove_files([
            "02_syntax_trees.mmd",
            "03_symbol_resolved_trees.mmd",
            "04_typed_trees.mmd",
            "00_timings.txt",
            "00_timings.html",
            "01_sources.txt",
            "01_sources.html",
            "02_ast.txt",
            "02_ast.html",
            "03_resolve.txt",
            "04_types.txt",
            "05_typed_program.txt",
            "06_validation.txt",
            "07_graph.txt",
            "08_proof.txt",
            "09_backend_plan.txt",
            "09_backend_report.txt",
            "09_backend_report.html",
            "09_native_plan.txt",
            "08_abstract_operations.html",
            "09_target_operations.html",
            "10_assigned_target_operations.html",
            "11_machine_instructions.html",
            "backend_report.html",
            "10_trust.txt",
            "10_trust.html",
            "11_emission.txt",
            "11_emission.html",
            "12_emission.txt",
            "12_emission.html",
            "12_emitted_output.txt",
            "12_emitted_output.html",
            "13_finalization.txt",
            "13_finalization.html",
            "13_emitted_output.txt",
            "13_emitted_output.html",
            "14_finalization.txt",
            "14_finalization.html",
        ])
        .map_err(|diagnostic| vec![diagnostic])
}

fn write_phase_json(
    options: &CompileOptions,
    file_name: &str,
    contents: &str,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_text(options, file_name, contents)
}

fn write_phase_diagram(
    options: &CompileOptions,
    file_name: &str,
    contents: &str,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_text(options, file_name, contents)
}

fn write_phase_text(
    options: &CompileOptions,
    file_name: &str,
    contents: &str,
) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_text(file_name, contents)
        .map_err(|diagnostic| vec![diagnostic])
}

fn json_diagnostic(error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "failed to serialize phase snapshot: {error}"
    ))]
}
