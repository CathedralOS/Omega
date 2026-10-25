//! Create real producing instances from the native product's exact companions.
//! This host root supplies the same invocation capabilities to both artifacts;
//! the interpreter owns child entry, suspension, completion and token custody.

use super::{TerminalRuntimeError, host_imports};
use terminal_interpreter::{
    TerminalComponentBinding, TerminalProgram, TerminalRuntimeSupplyRequest,
};

pub(super) fn prepare<'host, 'artifact>(
    component: &'artifact native_artifact::NativeComponentProduct,
    context: &host_imports::HostContext<'host>,
    x86_flags: Option<terminal_interpreter::TerminalX86FlagsEnvironment>,
) -> Result<TerminalComponentBinding<'host, 'artifact>, TerminalRuntimeError> {
    let artifact = component.artifact().view();
    let program = TerminalProgram::borrow(artifact);
    let runtime = component.runtime();
    if runtime.identity() != program.terminal_psi()
        || runtime.entry() != component.selection().candidate()
        || component.authority().subject() != program.terminal_psi()
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "component runtime companions name another producing occurrence",
        ));
    }
    let target_machines = terminal_interpreter::admit_target_machine_selections(
        artifact,
        runtime.target().target_name(),
    );
    let target_layouts = terminal_interpreter::admit_target_layouts(
        artifact,
        &target_machines,
        target::PhysicalLayoutContext::from_profile(runtime.target())
            .map(|context| context.data_layout()),
    )
    .map_err(|error| {
        TerminalRuntimeError::Requirements(
            terminal_interpreter::TerminalInterpretError::TargetLayoutSelection(error),
        )
    })?;
    let mut imports = Vec::new();
    for provision in runtime.host_provisions().host() {
        if matches!(&provision.realization,
            target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding)
                if matches!(binding.mechanism, target_operations::NormalizedForeignCallMechanism::Component { .. }))
        {
            return Err(TerminalRuntimeError::InvalidProvision(
                "nested component execution requires its own retained installed product roster",
            ));
        }
        if program
            .external_completion_requirement(provision.boundary)
            .is_none()
        {
            imports.push(provision.boundary);
        }
    }
    let supplies = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            x86_flags,
            entry: Some(terminal_psi::TargetMachineCallable::Machine(
                runtime.entry(),
            )),
            installation: runtime.host_provisions().installation(),
            host_imports: &imports,
            target_machines: Some(&target_machines),
            target_layouts: Some(&target_layouts),
        })
        .map_err(TerminalRuntimeError::Requirements)?;
    let mut binding =
        TerminalComponentBinding::new(component.selection().clone(), program.clone(), supplies)
            .map_err(TerminalRuntimeError::ComponentBinding)?;
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
