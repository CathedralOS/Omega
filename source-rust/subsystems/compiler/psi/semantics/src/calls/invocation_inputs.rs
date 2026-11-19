use semantic_vocabulary::MachineId;
use terminal_psi::{EvidenceContractLaneKind, TerminalModule};

pub fn validate_empty_erased_invocation_inputs(
    module: &TerminalModule,
    target: MachineId,
) -> Result<(), &'static str> {
    let mut targets = module
        .machines
        .iter()
        .filter(|machine| machine.id == target);
    let machine = targets
        .next()
        .ok_or("empty erased invocation target is absent")?;
    if targets.next().is_some() {
        return Err("empty erased invocation target is ambiguous");
    }
    if module.proofs.evidence_contract_lanes.iter().any(|lane| {
        lane.owner == terminal_psi::EvidenceCallableOwner::Machine(target)
            && lane.kind == EvidenceContractLaneKind::Requires
    }) {
        return Err("empty erased actual lane cannot supply declared requires evidence");
    }
    if !machine.contract.erased_scalar_formals.is_empty()
        || !machine.contract.erased_proof_formals.is_empty()
    {
        return Err("empty erased actual lane cannot supply declared proof formals");
    }
    let mut entries = machine
        .blocks
        .iter()
        .filter(|block| block.id == machine.entry);
    let entry = entries
        .next()
        .ok_or("empty erased invocation has no exact entry block")?;
    if entries.next().is_some() {
        return Err("empty erased invocation entry block is ambiguous");
    }
    if !entry.erased_scalar_formals.is_empty() || !entry.erased_proof_formals.is_empty() {
        return Err("empty erased actual lane cannot supply entry proof formals");
    }
    Ok(())
}
