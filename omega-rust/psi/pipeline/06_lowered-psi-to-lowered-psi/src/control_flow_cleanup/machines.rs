//! Optimizer module role: producer. Machine-level reachability pruning.
//!
//! Once per-machine folds have run, a machine no surviving transition can
//! reach is dead code: this pass drops it. Retention is the verifier's
//! `retained_machines` closure — the module entry, attached and ranked
//! machines, provider candidates, and every machine a module-level custody
//! or evidence row names are roots, and each retained machine keeps the
//! callees its surviving call, dispatch, and cleanup transitions name.
//!
//! Removal still refuses while evidence could dangle: a candidate whose
//! contents a row surviving the proposed removal still names — including the
//! unsealed checked-source and selected-IEEE sidecars only the producer can
//! see — stays in place. A machine kept that way is not a retention root,
//! but its own transitions still name machines: the keep-set grows until the
//! surviving-evidence check and the transition closure agree. The
//! independent verifier then re-derives the whole relation from the
//! rewritten module rather than trusting this proposal.

use semantic_vocabulary::{OperationId, ValueId};
use std::collections::BTreeSet;
use terminal_psi::{TerminalMachine, TerminalModule};

/// Drop every machine outside the keep-set the retained closure and the
/// surviving-evidence check converge on. Evidence is computed over the
/// module with all unreachable candidates removed, matching the relation the
/// independent verifier re-derives on the rewrite's output: a candidate the
/// remaining rows still pin stays unreachable but retained, and the machines
/// its own transitions name are pulled into the next round's closure.
pub(super) fn prune(
    module: &mut TerminalModule,
    sidecar_operations: &BTreeSet<OperationId>,
    sidecar_values: &BTreeSet<ValueId>,
) {
    let mut kept = terminal_verifier::retained_machines(module);
    loop {
        if module
            .machines
            .iter()
            .all(|machine| kept.contains(&machine.id))
        {
            return;
        }
        let mut proposed = module.clone();
        proposed
            .machines
            .retain(|machine| kept.contains(&machine.id));
        let evidence = terminal_verifier::block_local_evidence(&proposed);
        let bound = module.machines.iter().filter(|machine| {
            !kept.contains(&machine.id)
                && machine_bound(machine, &evidence, sidecar_operations, sidecar_values)
        });
        let next = terminal_verifier::retained_machines_with_roots(
            module,
            kept.iter().copied().chain(bound.map(|machine| machine.id)),
        );
        if next == kept {
            break;
        }
        kept = next;
    }
    module.machines.retain(|machine| kept.contains(&machine.id));
}

/// The verifier's surviving-evidence check plus the producer-only sidecar
/// pins: a recorded source call or selected IEEE occurrence keeps the machine
/// holding its named operation or captured scalar values.
fn machine_bound(
    machine: &TerminalMachine,
    evidence: &terminal_verifier::BlockLocalEvidence,
    sidecar_operations: &BTreeSet<OperationId>,
    sidecar_values: &BTreeSet<ValueId>,
) -> bool {
    if terminal_verifier::machine_evidence_bound(machine, evidence) {
        return true;
    }
    let sidecar_value = |value: &terminal_psi::ValueDeclaration| sidecar_values.contains(&value.id);
    if machine.parameters.iter().any(&sidecar_value)
        || machine.result.scalar_ref().is_some_and(&sidecar_value)
    {
        return true;
    }
    machine.blocks.iter().any(|block| {
        block.parameters.iter().any(&sidecar_value)
            || block.operations.iter().any(|operation| {
                sidecar_operations.contains(&operation.id)
                    || operation.result.scalar_ref().is_some_and(&sidecar_value)
            })
    })
}
