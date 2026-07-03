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

/// The PE optional-header Subsystem value the SELECTED target declares
/// (`subsystem console|gui|efi_application` in its target block; the word set
/// is validated at parse). Console (3) when no target is selected or the
/// selected target declares no subsystem.
fn resolved_target_subsystem(
    syntax_trees: &omega_syntax_trees::SyntaxTrees,
    target_name: Option<&str>,
) -> u16 {
    const CONSOLE: u16 = 3;
    let mut targets = syntax_trees.root_items().filter_map(|item| match item {
        omega_syntax_trees::item::Item::Target(target) => Some(target),
        _ => None,
    });
    let selected = match target_name {
        Some(target_name) => targets.find(|target| target.name.as_str() == target_name),
        // No target named on the command line: a program declaring exactly
        // ONE target states what it is; ambiguity falls back to console.
        None => match (targets.next(), targets.next()) {
            (Some(only), None) => Some(only),
            _ => None,
        },
    };
    match selected
        .and_then(|target| target.subsystem.as_ref())
        .map(|word| word.as_str())
    {
        Some("gui") => 2,
        Some("efi_application") => 10,
        _ => CONSOLE,
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
        crate::pipeline::wire_plans::compute_wire_plans(&mut typed);
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
        let subsystem = resolved_target_subsystem(
            &syntax_trees,
            self.options.target_name.as_deref(),
        );
        const EFI_APPLICATION: u16 = 10;
        let freestanding = subsystem == EFI_APPLICATION;

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
