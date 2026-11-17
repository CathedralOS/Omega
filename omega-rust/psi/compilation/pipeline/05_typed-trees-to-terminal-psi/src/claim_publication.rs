//! Describe logical claim names and transports after storage lowering. The
//! rows confer no checked status: Psi 06 reconstructs them from its own input.

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::TerminalModule;

fn preparation_error(
    machine: MachineId,
    error: terminal_semantics::plain_affine::Error,
) -> Refusal {
    let concerns = match error.site() {
        Some(terminal_semantics::plain_affine::Site::Operation(at)) => Construct::Operation(at),
        Some(terminal_semantics::plain_affine::Site::Edge(at)) => Construct::Edge(at),
        None => Construct::Machine,
    };
    let (reason, detail) = match error {
        terminal_semantics::plain_affine::Error::Unsupported { detail, .. } => {
            (RefusalReason::Unsupported, detail)
        }
        terminal_semantics::plain_affine::Error::Malformed { detail, .. } => {
            (RefusalReason::ForeignIdentity, detail)
        }
    };
    Refusal {
        reason,
        machine,
        concerns,
        source_span: None,
        detail: format!("preparing logical claim transport: {detail}"),
    }
}

pub(crate) fn publish(diagnostic: MachineId, module: &mut TerminalModule) -> Checked<()> {
    // Complete every public header before deriving call rows. The body and a
    // selected requirement can expose different contracts; neither borrows the
    // other's private promises. Boundary custody is separately authenticated.
    for contract in &mut module.declaration_contracts {
        terminal_semantics::plain_affine::publish_contract_claims(
            contract,
            &module.structural_types,
        )
        .map_err(|error| preparation_error(diagnostic, error))?;
    }
    for machine in &mut module.machines {
        terminal_semantics::plain_affine::publish_contract_claims(
            &mut machine.contract,
            &module.structural_types,
        )
        .map_err(|error| preparation_error(machine.id, error))?;
    }
    for ordinal in 0..module.machines.len() {
        let identity = module.machines[ordinal].id;
        let plan = terminal_semantics::plain_affine::plan_claims_in(module, identity)
            .map_err(|error| preparation_error(identity, error))?;
        plan.install(&mut module.machines[ordinal])
            .map_err(|error| preparation_error(identity, error))?;
    }
    Ok(())
}
