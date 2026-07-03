use crate::pipeline::artifacts::{
    remove_stale_phase_diagrams, write_backend_report, write_checked_snapshot,
    write_control_flow_snapshot, write_emission_plan, write_pipeline_index, write_pipeline_shell,
    write_resolved_snapshot, write_state_graph_snapshot, write_syntax_snapshot, write_timings,
    write_typed_snapshot,
};
use crate::pipeline::boundary_report::{
    write_boundary_report, write_boundary_report_with_capabilities,
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

/// Builds the boundary provider registry from `provider` declarations, enforces
/// the package whitelist, and rejects boundary operator bindings that do not
/// resolve to a registered provider (frozen Wave 0 decision #4).
fn validate_boundary_providers(
    syntax: &omega_syntax_trees::SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let registry = omega_effects::build_provider_registry(syntax, &mut diagnostics);
    omega_effects::validate_provider_bindings(syntax, &registry, &mut diagnostics);

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
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

        let (source_file_count, mut syntax) = source_files_to_syntax_trees(
            &self.options.root_path,
            self.options.target_name.as_deref(),
            &mut timings,
        )?;
        // PLAN-LAID VALUE TYPES (layouts L4), desugar half: synthesize the
        // `Policy<Schema>` instance definitions before resolution so every
        // later stage sees ordinary records.
        let plan_laid_records =
            crate::pipeline::plan_laid::desugar_plan_laid_value_types(&mut syntax.syntax_trees)?;
        remove_stale_phase_diagrams(&self.options)?;
        write_pipeline_index(&self.options)?;
        write_syntax_snapshot(&self.options, &syntax)?;
        write_boundary_report(&self.options, &syntax.syntax_trees)?;
        validate_boundary_providers(&syntax.syntax_trees)?;
        let syntax_trees = syntax.syntax_trees.clone();

        let resolved = syntax_trees_to_symbol_resolved_trees(syntax, &mut timings)?;
        write_resolved_snapshot(&self.options, &resolved)?;

        let mut typed = symbol_resolved_trees_to_typed_trees(resolved, &mut timings)?;
        // COMPTIME STAGE 1: evaluate effect-free machine calls in fixed-array
        // length position and substitute concrete literals BEFORE checking,
        // proof facts, and layout consume the lengths.
        crate::pipeline::const_lengths::evaluate_const_array_lengths(&mut typed)?;
        // PLAN-LAID VALUE TYPES, plan half: evaluate + validate each policy
        // application and record the placements for the layout builder.
        crate::pipeline::plan_laid::compute_plan_laid_layouts(&mut typed, &plan_laid_records)?;
        // WIRE PLANS (mint arc rung 2a): derive each numbered schema's
        // placement plan; the wire codec selection consumes it (tag + framing
        // from the plan, asserted against its own walk).
        crate::pipeline::wire_plans::compute_wire_plans(&mut typed)?;
        // BUILD CONFIG (build_and_package_model.md): image facts from
        // build.omg's augmenting `build(b: &mut Build)` machine, evaluated at
        // build time. When present it is AUTHORITATIVE; the legacy in-source
        // `target { subsystem }` word is the fallback until its removal.
        let build_config = crate::pipeline::build_config::compute_build_config(&typed)?;
        let build_machine_present = typed
            .machines()
            .iter()
            .any(|machine| machine.name.as_str() == "build");
        write_typed_snapshot(&self.options, &typed)?;
        crate::pipeline::wire_report::write_wire_protocol_report(&self.options, &typed)?;

        let checked = typed_trees_to_checked_trees(typed, &mut timings)?;
        write_checked_snapshot(&self.options, &checked.program)?;
        write_boundary_report_with_capabilities(&self.options, &syntax_trees, &checked.program)?;
        let backend_surface = build_backend_surface_report(&checked.program);

        let state_graph = checked_trees_to_state_graph(&checked, workers.handle(), &mut timings)?;
        write_state_graph_snapshot(&self.options, &state_graph)?;
        let control_flow = state_graph_to_control_flow(state_graph, &mut timings)?;
        write_control_flow_snapshot(&self.options, &control_flow)?;

        // The selected target's declared image subsystem (`subsystem
        // console|gui|efi_application`, ch: target blocks); console when the
        // target declares none. PE consumes it; other formats ignore it.
        // Resolved BEFORE the backend build because `efi_application` also
        // means FREESTANDING: the target trusts no host boundary packages, so
        // the backend builds against an empty host ABI plan (no bindings, no
        // import thunks -- services arrive via the entry's parameters).
        // Image facts come from build.omg (build_and_package_model.md); the
        // in-source `target { subsystem }` word is retired.
        let _ = build_machine_present;
        let (subsystem, freestanding) = (build_config.subsystem, build_config.freestanding);

        let backend = control_flow_to_backend_plan(
            checked,
            self.options.target_name.as_deref(),
            freestanding,
            control_flow,
            workers.handle(),
            &mut timings,
        )?;
        if self.options.write_output {
            write_backend_report(&self.options, &backend_surface, &backend.plan)?;
        }

        let (emission_plan, emitted) =
            backend_plan_to_native_image_payload(&backend, subsystem, &mut timings)?;

        if self.options.write_output {
            let output_path = write_output(&self.options, emitted)?;
            write_emission_plan(
                &self.options,
                &backend.plan,
                &emission_plan,
                Some(output_path.as_path()),
            )?;
            write_timings(&self.options, timings.as_slice())?;
        } else {
            write_emission_plan(&self.options, &backend.plan, &emission_plan, None)?;
        }

        write_pipeline_shell(&self.options)?;

        Ok(CompileReport {
            root_path: self.options.root_path,
            source_file_count,
            wrote_output: self.options.write_output,
        })
    }
}
