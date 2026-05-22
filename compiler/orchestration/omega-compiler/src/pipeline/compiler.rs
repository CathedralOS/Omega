use crate::pipeline::artifacts::{
    remove_stale_phase_diagrams, write_backend_report, write_control_flow_snapshot,
    write_emission_plan, write_pipeline_index, write_pipeline_shell, write_resolved_snapshot,
    write_state_graph_snapshot, write_syntax_snapshot, write_timings, write_typed_snapshot,
};
use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::compile_report::CompileReport;
use crate::pipeline::output::write_output;
use crate::pipeline::stages::{
    backend_plan_to_native_image_payload, checked_trees_to_state_graph,
    control_flow_to_backend_plan, source_files_to_syntax_trees, state_graph_to_control_flow,
    symbol_resolved_trees_to_typed_trees, syntax_trees_to_symbol_resolved_trees,
    typed_trees_to_checked_trees,
};
use crate::pipeline::timing::CompileTimings;
use omega_artifacts::build_backend_surface_report;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPool;

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
        let workers = WorkerPool::with_available_parallelism();
        let mut timings = CompileTimings::default();

        let (source_file_count, syntax) = source_files_to_syntax_trees(
            &self.options.root_path,
            self.options.target_name.as_deref(),
            &mut timings,
        )?;
        remove_stale_phase_diagrams(&self.options)?;
        write_pipeline_index(&self.options)?;
        write_syntax_snapshot(&self.options, &syntax)?;

        let resolved = syntax_trees_to_symbol_resolved_trees(syntax, &mut timings)?;
        write_resolved_snapshot(&self.options, &resolved)?;

        let typed = symbol_resolved_trees_to_typed_trees(resolved, &mut timings)?;
        write_typed_snapshot(&self.options, &typed)?;

        let checked = typed_trees_to_checked_trees(typed, &mut timings)?;
        let backend_surface = build_backend_surface_report(&checked.program);

        let state_graph = checked_trees_to_state_graph(&checked, workers.handle(), &mut timings)?;
        write_state_graph_snapshot(&self.options, &state_graph)?;
        let control_flow = state_graph_to_control_flow(state_graph, &mut timings)?;
        write_control_flow_snapshot(&self.options, &control_flow)?;

        let backend = control_flow_to_backend_plan(
            checked,
            self.options.target_name.as_deref(),
            control_flow,
            workers.handle(),
            &mut timings,
        )?;

        let (emission_plan, emitted) =
            backend_plan_to_native_image_payload(&backend, &mut timings)?;

        if self.options.write_output {
            write_backend_report(&self.options, &backend_surface, &backend.plan)?;
            write_emission_plan(&self.options, &emission_plan)?;
        }

        if self.options.write_output {
            write_output(&self.options, emitted)?;
            write_timings(&self.options, timings.as_slice())?;
        }

        write_pipeline_shell(&self.options)?;

        Ok(CompileReport {
            root_path: self.options.root_path,
            source_file_count,
            wrote_output: self.options.write_output,
        })
    }
}
