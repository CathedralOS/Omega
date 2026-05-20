use crate::pipeline::source::SourceStorage;
use omega_checked_trees::Program as CheckedProgram;
use omega_core::diagnostics::Diagnostic;
use omega_emission_planning::{EmissionPlanningInput, build_emission_plan};
use omega_object::SectionKind;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_syntax_trees::SyntaxTrees;
use omega_target::NativeTarget;
use omega_typed_trees::TypedTrees;
use std::sync::Arc;

pub(super) struct AssembledSyntax {
    pub(super) syntax_trees: SyntaxTrees,
    pub(super) sources: Arc<omega_core::source::SourceMap>,
}

pub(super) struct CheckedProgramSurface {
    pub(super) program: CheckedProgram,
}

pub(super) struct EmittedProgram {
    pub(super) target: NativeTarget,
    pub(super) planned_text_bytes: usize,
    pub(super) object: omega_object::ObjectPlan,
    pub(super) relocations: omega_object::RelocationPlan,
    pub(super) text_bytes: Vec<u8>,
    pub(super) data_bytes: Vec<u8>,
}

pub(super) fn assemble_syntax(sources: SourceStorage) -> Result<AssembledSyntax, Vec<Diagnostic>> {
    Ok(AssembledSyntax {
        syntax_trees: sources.syntax_trees,
        sources: Arc::new(sources.sources),
    })
}

pub(super) fn resolve_program(
    syntax: AssembledSyntax,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax.syntax_trees,
        syntax.sources,
    )
    .map_err(|diagnostic| vec![diagnostic])
}

pub(super) fn typecheck_program(
    resolved: SymbolResolvedTrees,
) -> Result<TypedTrees, Vec<Diagnostic>> {
    omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_owned(resolved)
        .map_err(|diagnostic| vec![diagnostic])
}

pub(super) fn check_program(typed: TypedTrees) -> Result<CheckedProgramSurface, Vec<Diagnostic>> {
    let program = omega_typed_trees_to_checked_trees::lower_typed_trees(typed)?;
    Ok(CheckedProgramSurface { program })
}

pub(super) fn plan_backend(
    checked: CheckedProgramSurface,
    target_name: Option<&str>,
    workers: omega_core::parallel::WorkerPoolHandle,
) -> Result<omega_backend_plan::BackendPlan, Vec<Diagnostic>> {
    let target =
        NativeTarget::from_omega_target_name(target_name).map_err(|diagnostic| vec![diagnostic])?;
    let checked_program = Arc::new(checked.program);
    let state_graph = omega_checked_trees_to_state_graph::build_state_graph_with_workers(
        Arc::clone(&checked_program),
        workers.clone(),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let control_flow =
        omega_state_graph_to_control_flow::build_control_flow_plan_owned(state_graph)
            .map_err(|diagnostic| vec![diagnostic])?;

    omega_backend_pipeline::build_backend_plan_from_control_flow_with_workers(
        checked_program,
        target,
        Arc::new(control_flow),
        workers,
    )
    .map_err(|diagnostic| vec![diagnostic])
}

pub(super) fn plan_emission(
    plan: &omega_backend_plan::BackendPlan,
) -> omega_artifacts::EmissionPlan {
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

pub(super) fn ensure_emission_ready(
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

pub(super) fn emit_backend(
    plan: &omega_backend_plan::BackendPlan,
) -> Result<EmittedProgram, Vec<Diagnostic>> {
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

fn object_text_size(object: &omega_object::ObjectPlan) -> usize {
    object
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Text)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}
