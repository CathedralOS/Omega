// Prepare real producing component instances (`TerminalComponentBinding`) from
// a native product's exact runtime companions. The host root supplies the same
// invocation capabilities to both artifacts while the interpreter owns child
// entry, suspension, completion, and token custody.

use super::{TerminalRuntimeError, host_imports};
use terminal_interpreter::{
    TerminalComponentBinding, TerminalInterpretError, TerminalProgram,
    TerminalRuntimeSupplyRequest, admit_target_layouts, terminal_target_context,
};

pub(super) fn prepare<'host, 'artifact>(
    component: &'artifact published_runtime_input::RuntimeComponent,
    provisions: &'artifact published_runtime_input::RuntimeExecutionProvisions,
    context: &host_imports::HostContext<'host>,
    x86_flags: Option<terminal_interpreter::TerminalX86FlagsEnvironment>,
    memory_ordering: Option<terminal_interpreter::TerminalMemoryOrderingEnvironment>,
) -> Result<TerminalComponentBinding<'host, 'artifact>, TerminalRuntimeError> {
    let artifact = component.artifact().view();
    let program = TerminalProgram::borrow(artifact).map_err(|_| {
        TerminalRuntimeError::InvalidProvision(
            "received component Psi requires the host's authorization of its exact artifact",
        )
    })?;
    let runtime = component.runtime();
    if runtime.identity() != program.terminal_psi()
        || runtime.entry() != component.selection().candidate()
        || component.authority().subject() != program.terminal_psi()
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "component runtime companions name another producing occurrence",
        ));
    }
    let target_machines = terminal_target_context(artifact, runtime.target().target_name());
    let facts = super::layout_target_facts(runtime.target().target_name()).map_err(|error| {
        TerminalRuntimeError::Requirements(TerminalInterpretError::TargetLayoutSelection(error))
    })?;
    let target_layouts =
        admit_target_layouts(artifact, &target_machines, &facts).map_err(|error| {
            TerminalRuntimeError::Requirements(TerminalInterpretError::TargetLayoutSelection(error))
        })?;
    let children = provisions
        .component_dependencies(component)
        .map_err(TerminalRuntimeError::InvalidProvision)?;
    let mut host_boundaries = host_imports::HostBoundarySelections::default();
    for provision in runtime.host_provisions().host() {
        if matches!(&provision.realization,
            target_vocabulary::BoundarySettlementRealization::NormalizedForeignCall(binding)
                if matches!(binding.mechanism, target_vocabulary::NormalizedForeignCallMechanism::Component { .. }))
        {
            continue;
        }
        host_boundaries.add(&program, provision);
    }
    let mut supplies = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            native_code_layouts: None,
            x86_flags,
            memory_ordering,
            entry: Some(terminal_psi::TargetMachineCallable::Machine(
                runtime.entry(),
            )),
            installation: runtime
                .scalar_applications()
                .map(|completed| completed.primary().providers())
                .or_else(|| runtime.host_provisions().installation()),
            host_imports: &host_boundaries.imports,
            external_completions: &host_boundaries.external_completions,
            target_machines: Some(&target_machines),
            target_layouts: Some(&target_layouts),
        })
        .map_err(TerminalRuntimeError::Requirements)?;
    for child in &children {
        supplies = supplies
            .with_component_provider(child.selection())
            .map_err(TerminalRuntimeError::Requirements)?;
    }
    let mut binding = TerminalComponentBinding::new(
        component.selection().clone(),
        program.clone(),
        supplies,
        runtime
            .scalar_applications()
            .map(std::borrow::Cow::Borrowed),
    )
    .map_err(TerminalRuntimeError::ComponentBinding)?;
    for child in children {
        let dependency = prepare(child, provisions, context, x86_flags, memory_ordering)?;
        binding
            .bind_component(dependency)
            .map_err(TerminalRuntimeError::ComponentBinding)?;
    }
    let imports = binding.unbound_imports().collect::<Vec<_>>();
    host_imports::bind(
        &program,
        &mut binding,
        Some(runtime.host_provisions()),
        &imports,
        context,
    )?;
    Ok(binding)
}
