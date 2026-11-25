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
        terminal_semantics::plain_affine::Error::Unavailable(unavailable) => {
            (RefusalReason::Unestablished, unavailable.message())
        }
        terminal_semantics::plain_affine::Error::Unconsumed { at_join, .. } => (
            RefusalReason::Violation,
            terminal_semantics::plain_affine::Error::unconsumed_message(at_join),
        ),
        terminal_semantics::plain_affine::Error::Unsupported { detail, .. } => {
            (RefusalReason::Unsupported, detail)
        }
        terminal_semantics::plain_affine::Error::Malformed { detail, .. } => {
            (RefusalReason::ForeignIdentity, detail)
        }
    };
    Refusal {
        reason,
        site: terminal_check_tables::RefusalSite::Machine(machine, concerns),
        source_span: None,
        detail: format!("preparing logical claim transport: {detail}"),
    }
}

pub(crate) fn publish(diagnostic: MachineId, module: &mut TerminalModule) -> Checked<()> {
    // Complete every public header before deriving call rows. The body and a
    // selected requirement can expose different contracts; neither borrows the
    // other's private promises. Boundary custody is separately authenticated.
    for (machine, contract) in module
        .execution
        .declaration_contracts
        .iter_mut()
        .map(|contract| (diagnostic, contract))
        .chain(
            module
                .machines
                .iter_mut()
                .map(|machine| (machine.id, &mut machine.contract)),
        )
    {
        terminal_semantics::plain_affine::publish_contract_claims(
            contract,
            &module.types.structural_types,
        )
        .map_err(|error| preparation_error(machine, error))?;
        terminal_semantics::content_transformations::entry_claims::publish_declared_entry_contents(
            &module.types.structural_types,
            &module.types.structural_domains,
            &module.identities,
            contract,
        )
        .map_err(|detail| Refusal {
            reason: RefusalReason::ForeignIdentity,
            site: terminal_check_tables::RefusalSite::Machine(machine, Construct::Machine),
            source_span: None,
            detail: detail.into(),
        })?;
    }
    for ordinal in 0..module.machines.len() {
        let identity = module.machines[ordinal].id;
        let plan = terminal_semantics::plain_affine::plan_claims_in(module, identity)
            .map_err(|error| preparation_error(identity, error))?;
        plan.install(&mut module.machines[ordinal])
            .map_err(|error| preparation_error(identity, error))?;
    }
    if !terminal_semantics::content_transformations::boundary_calls::has_declared_conservation(
        module,
    ) {
        return Ok(());
    }
    for ordinal in 0..module.machines.len() {
        let caller = &module.machines[ordinal];
        let mut rows = Vec::new();
        for operation in caller.operations() {
            // An affine value may carry a declared mathematical law without owning
            // any resource claim. That case still lowers for declaration checking.
            // Stage 06 independently requires evidence at each actual invocation.
            if !matches!(&operation.kind, terminal_psi::OperationKind::BoundaryCall { completion_receipts, .. } if !completion_receipts.is_empty())
                || !operation
                    .result
                    .as_structural()
                    .is_some_and(|result| !result.claims.is_empty())
            {
                continue;
            }
            rows.extend(
                terminal_semantics::content_transformations::boundary_calls::compositions(
                    module, caller, operation,
                )
                .map_err(|detail| Refusal {
                    reason: RefusalReason::Unsupported,
                    site: terminal_check_tables::RefusalSite::Machine(
                        caller.id,
                        Construct::Operation(operation.id),
                    ),
                    source_span: None,
                    detail: detail.into(),
                })?,
            );
        }
        rows.sort_unstable();
        module.machines[ordinal]
            .content_partition_compositions
            .extend(rows);
    }
    Ok(())
}
