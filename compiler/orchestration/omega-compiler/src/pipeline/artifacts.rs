use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{ArtifactWriter, PhaseTiming};
use omega_backend_report::{backend_report_text, BackendReportInput, BackendReportPhaseTiming};
use omega_core::diagnostics::Diagnostic;

pub(super) fn write_pipeline_index(options: &CompileOptions) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "00_pipeline.html",
        &omega_visualizations::pipeline_index_html(),
    )
}

pub(super) fn write_syntax_snapshot(
    options: &CompileOptions,
    syntax: &omega_syntax_trees::SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    write_phase_diagram(
        options,
        "02_syntax_trees.html",
        &omega_visualizations::syntax_trees_html(syntax),
    )?;
    write_phase_json(
        options,
        "02_syntax_trees.json",
        &syntax.snapshot_json_pretty().map_err(json_diagnostic)?,
    )
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
    let output_path = build_dir.join("09_backend_report.html");
    std::fs::create_dir_all(build_dir).map_err(io_diagnostic)?;
    std::fs::write(
        output_path,
        omega_visualizations::text_report_html("backend_report", &report),
    )
    .map_err(io_diagnostic)
}

pub(super) fn write_emission_plan(
    options: &CompileOptions,
    emission_plan: &omega_artifacts::EmissionPlan,
) -> Result<(), Vec<Diagnostic>> {
    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_emission_plan(emission_plan)
        .map_err(|diagnostic| vec![diagnostic])
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
            "01_sources.txt",
            "02_ast.txt",
            "03_resolve.txt",
            "04_types.txt",
            "05_typed_program.txt",
            "06_validation.txt",
            "07_graph.txt",
            "08_proof.txt",
            "09_backend_plan.txt",
            "09_backend_report.txt",
            "09_native_plan.txt",
            "10_trust.txt",
            "11_emission.txt",
            "12_emitted_output.txt",
            "13_finalization.txt",
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

fn io_diagnostic(error: std::io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::error(error.to_string())]
}
