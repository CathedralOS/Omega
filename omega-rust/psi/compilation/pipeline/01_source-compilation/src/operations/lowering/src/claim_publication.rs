//! Prepare claim declarations before storage planning needs their exact
//! obligations. The ownership walk emits transports with the cleanup plan;
//! Psi 06 independently reconstructs both.

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::TerminalModule;

pub(crate) fn preparation_error(
    machine: MachineId,
    error: terminal_semantics::plain_affine::Error,
) -> Refusal {
    let concerns = match error.site() {
        Some(terminal_semantics::plain_affine::Site::Operation(at)) => Construct::Operation(at),
        Some(terminal_semantics::plain_affine::Site::Edge(at)) => Construct::Edge(at),
        None => Construct::Machine,
    };
    let (reason, detail) = match error {
        terminal_semantics::plain_affine::Error::Unavailable(unavailable) => {
            (RefusalReason::Unestablished, unavailable.message())
        }
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

pub(crate) fn prepare(diagnostic: MachineId, module: &mut TerminalModule) -> Checked<()> {
    let content = terminal_semantics::content_claims::ClaimProjections::new(
        &module.structural_types,
        &module.structural_domains,
        &module.identities,
    )
    .map_err(|detail| {
        preparation_error(
            diagnostic,
            terminal_semantics::plain_affine::Error::Unsupported { detail, site: None },
        )
    })?;
    // Complete every public header before deriving call rows. The body and a
    // selected requirement can expose different contracts; neither borrows the
    // other's private promises. Boundary custody is separately authenticated.
    for contract in &mut module.declaration_contracts {
        terminal_semantics::plain_affine::publish_contract_claims(
            contract,
            &module.structural_types,
        )
        .map_err(|error| preparation_error(diagnostic, error))?;
        contract.content_entry_claims = content.entry_claims(contract).map_err(|detail| {
            preparation_error(
                diagnostic,
                terminal_semantics::plain_affine::Error::Unsupported { detail, site: None },
            )
        })?;
    }
    for machine in &mut module.machines {
        terminal_semantics::plain_affine::publish_contract_claims(
            &mut machine.contract,
            &module.structural_types,
        )
        .map_err(|error| preparation_error(machine.id, error))?;
        machine.contract.content_entry_claims =
            content.entry_claims(&machine.contract).map_err(|detail| {
                preparation_error(
                    machine.id,
                    terminal_semantics::plain_affine::Error::Unsupported { detail, site: None },
                )
            })?;
    }
    Ok(())
}

/// Resolve explicit requests before descriptor liveness reads their real
/// custody operands. The later joint walk still owns the final cleanup plan.
pub(crate) fn materialize_content(module: &mut TerminalModule) -> Checked<()> {
    for ordinal in 0..module.machines.len() {
        let machine = &module.machines[ordinal];
        if !machine.operations().any(|operation| {
            matches!(&operation.kind,
            terminal_psi::OperationKind::TransformContent { derivations, .. }
                if derivations.iter().any(|row| row.inputs.is_empty() || row.outputs.is_empty()))
        }) {
            continue;
        }
        let owner = machine.id;
        let claims = terminal_semantics::plain_affine::plan_claims_in(module, owner)
            .map_err(|error| preparation_error(owner, error))?;
        claims
            .install(&mut module.machines[ordinal])
            .map_err(|error| preparation_error(owner, error))?;
    }
    Ok(())
}
