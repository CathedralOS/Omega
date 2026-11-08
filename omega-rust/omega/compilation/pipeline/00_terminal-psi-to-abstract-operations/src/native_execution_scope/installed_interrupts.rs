//! Installation invokes its checked validator and work body and retains its
//! completion policy. Those choices belong to the exact operation, even though
//! the core boundary has no ordinary source body containing their calls.

use semantic_vocabulary::{MachineId, OperationId};
use terminal_psi::TerminalModule;

pub(super) fn helper_roots(
    module: &TerminalModule,
    operation: OperationId,
) -> Result<Vec<MachineId>, String> {
    if !module
        .installed_interrupt_registrations
        .iter()
        .any(|registration| registration.operation == operation)
    {
        return Ok(Vec::new());
    }
    let mut roots = Vec::with_capacity(3);
    // The authenticated core declaration's Machine telescope is Valid,
    // Validate, Work, Completion. Valid is a logical denotation; the other
    // three are nominal contracts whose selected bodies are retained here.
    for ordinal in [1, 2, 3] {
        let mut selections = module.callback_applications.iter().filter(|selection| {
            selection.registrar == operation && selection.static_machine_ordinal == ordinal
        });
        let (Some(selection), None) = (selections.next(), selections.next()) else {
            return Err(format!(
                "interrupt installation {operation:?} has no unique checked helper at ordinal {ordinal}"
            ));
        };
        let mut bodies = module.machines.iter().filter(|machine| {
            machine.declaration.as_ref().is_some_and(|declaration| {
                declaration.same_identity_as(
                    &module.identities,
                    &selection.body,
                    &module.identities,
                )
            })
        });
        let (Some(body), None) = (bodies.next(), bodies.next()) else {
            return Err(format!(
                "interrupt installation {operation:?} helper {ordinal} has no unique retained body"
            ));
        };
        roots.push(body.id);
    }
    Ok(roots)
}
