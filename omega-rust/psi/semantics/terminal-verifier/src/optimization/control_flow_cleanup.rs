//! Validation of control flow cleanup rewrites.

use crate::optimization::{
    block_local_evidence, evidence_bound_block, machine_evidence_bound, retained_machines,
};
use crate::{
    ModuleError, reconstruct_optimizable_terminal_obligations, validate_module_for_optimization,
};
use semantic_vocabulary::{BlockId, MachineId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{TerminalModule, Terminator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlFlowCleanupRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedMachine(MachineId),
    ChangedSurvivingBlock(BlockId),
    UnjustifiedFold(BlockId),
    RemovedReachableBlock(BlockId),
    RemovedEvidenceBlock(BlockId),
    RemovedReachableMachine(MachineId),
    RemovedEvidenceMachine(MachineId),
    ChangedProofQuestion,
}

/// Check the exact branch-cleanup relation, not the producer's folding order.
///
/// A rewritten terminator is justified only when the `before` conditional's
/// condition is the result of a `BooleanConstant` operation in `before`: the
/// selected successor edge — identity, scalar and structural bindings, and
/// edge-scoped cleanup rows — is carried verbatim onto an unconditional
/// `Jump` with an empty residual cleanup list, and the untaken successor
/// disappears with the conditional. Every other surviving block row is
/// identical; no block is added, reordered, or re-entered.
///
/// A removed block is justified only when it is unreachable under the
/// performed folds and carries nothing evidence retains: no static reach
/// binding, no structural result or parameter row whose machine-level place
/// declaration would orphan, and no identity the module's contract,
/// crash-contract, qualification, invariant, suspension, dispatch,
/// call-evidence, or projection carriers name. Removing any such row would
/// leave evidence pointing at structure the module no longer contains.
///
/// A `before` module carrying reconstructed proof obligations admits only a
/// rewrite the question carries verbatim: the unchanged-question check above
/// is the refusal boundary, so a fold that leaves the question intact is
/// checked by the same cleanup relation while a fold that perturbs it fails
/// `ChangedProofQuestion` until proof-context transport is implemented.
/// Ranked machines carry ranking evidence over exact control positions and
/// pass through unchanged.
///
/// The rewrite may additionally drop whole machines: `after` keeps
/// `before`'s machine order with a subsequence of its rows. A removed machine
/// is justified only when no retained transition reaches it — the entry
/// machine, every provider candidate, every attached machine, and every
/// machine a surviving module-level custody or evidence row names are roots,
/// and each retained machine keeps the machines its surviving blocks call,
/// dispatch, or clean up — and when no surviving evidence row names a block,
/// edge, operation, or value inside it. The check re-derives retention from
/// `after`: a machine reachable only through removed machines leaves, while
/// one named by a surviving row or a surviving machine's transition does not.
pub fn validate_control_flow_cleanup(
    before: &TerminalModule,
    after: &TerminalModule,
) -> Result<(), ControlFlowCleanupRewriteError> {
    use ControlFlowCleanupRewriteError as RewriteError;
    let before_valid =
        validate_module_for_optimization(before).map_err(RewriteError::InvalidModule)?;
    let after_valid =
        validate_module_for_optimization(after).map_err(RewriteError::InvalidModule)?;
    // Machines leave only by removal: `after` carries an ordered subsequence
    // of `before`'s rows, so a cursor match separates surviving pairs from
    // removed machines and rejects added or reordered rows at once.
    let mut surviving_pairs = Vec::new();
    let mut removed_machines = Vec::new();
    let mut incoming = after.machines.iter().peekable();
    for old in &before.machines {
        if incoming.peek().is_some_and(|new| new.id == old.id) {
            surviving_pairs.push((old, incoming.next().expect("peeked row")));
        } else {
            removed_machines.push(old);
        }
    }
    if incoming.next().is_some() {
        return Err(RewriteError::ChangedProgramStructure);
    }
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(RewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(RewriteError::InvalidModule)?;
    if old_question != new_question {
        return Err(RewriteError::ChangedProofQuestion);
    }
    let evidence = block_local_evidence(before);
    for (old, new) in surviving_pairs {
        if old.id != new.id {
            return Err(RewriteError::ChangedProgramStructure);
        }
        let mut non_blocks = new.clone();
        non_blocks.blocks.clone_from(&old.blocks);
        if &non_blocks != old {
            return Err(RewriteError::ChangedMachine(old.id));
        }
        if old.ranked_scc.is_some() {
            if new != old {
                return Err(RewriteError::ChangedMachine(old.id));
            }
            continue;
        }
        // `after` is already validated, so every surviving block's condition
        // still resolves: a literal producer inside a removed block can never
        // feed a surviving conditional.
        let mut literals = BTreeMap::new();
        for block in &old.blocks {
            for operation in &block.operations {
                if let terminal_psi::OperationKind::BooleanConstant { value } = &operation.kind
                    && let Some(result) = operation.result.scalar()
                {
                    literals.insert(result.id, *value);
                }
            }
        }
        let mut folded = BTreeMap::new();
        let mut old_blocks = old.blocks.iter();
        for new_block in &new.blocks {
            let Some(old_block) = old_blocks.find(|block| block.id == new_block.id) else {
                return Err(RewriteError::ChangedProgramStructure);
            };
            if new_block == old_block {
                continue;
            }
            if new_block.parameters != old_block.parameters
                || new_block.structural_parameters != old_block.structural_parameters
                || new_block.operations != old_block.operations
            {
                return Err(RewriteError::ChangedSurvivingBlock(old_block.id));
            }
            let Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } = &old_block.terminator
            else {
                return Err(RewriteError::UnjustifiedFold(old_block.id));
            };
            let taken = match literals.get(condition) {
                Some(true) => when_true,
                Some(false) => when_false,
                None => return Err(RewriteError::UnjustifiedFold(old_block.id)),
            };
            let expected = Terminator::Jump {
                edge: taken.edge,
                target: taken.target,
                arguments: taken.arguments.clone(),
                erased_arguments: taken.erased_arguments.clone(),
                structural_arguments: taken.structural_arguments.clone(),
                trivial_affine_discards: taken.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
            };
            if new_block.terminator != expected {
                return Err(RewriteError::UnjustifiedFold(old_block.id));
            }
            folded.insert(old_block.id, new_block.terminator.clone());
        }
        let mut simulated = old.clone();
        for block in &mut simulated.blocks {
            if let Some(terminator) = folded.get(&block.id) {
                block.terminator = terminator.clone();
            }
        }
        let outgoing = crate::control_graph::successors(&simulated);
        let mut reachable = BTreeSet::from([simulated.entry]);
        let mut pending = vec![simulated.entry];
        while let Some(block) = pending.pop() {
            for (_, target) in &outgoing[&block] {
                if reachable.insert(*target) {
                    pending.push(*target);
                }
            }
        }
        let surviving: BTreeSet<BlockId> = new.blocks.iter().map(|block| block.id).collect();
        for old_block in &old.blocks {
            if surviving.contains(&old_block.id) {
                continue;
            }
            if reachable.contains(&old_block.id) {
                return Err(RewriteError::RemovedReachableBlock(old_block.id));
            }
            if evidence_bound_block(old_block, &evidence) {
                return Err(RewriteError::RemovedEvidenceBlock(old_block.id));
            }
        }
    }
    if !removed_machines.is_empty() {
        // Retention is re-derived over `after`: only surviving rows and
        // surviving machines can keep a machine alive, and their own
        // transitions then close the set. A machine a removed sibling still
        // named is dead with it; a machine any surviving row or reachable
        // call, dispatch, or cleanup transition names is not.
        let retained = retained_machines(after);
        let surviving_evidence = block_local_evidence(after);
        for machine in &removed_machines {
            if retained.contains(&machine.id) {
                return Err(RewriteError::RemovedReachableMachine(machine.id));
            }
            if machine_evidence_bound(machine, &surviving_evidence) {
                return Err(RewriteError::RemovedEvidenceMachine(machine.id));
            }
        }
    }
    let mut non_machines = after.clone();
    non_machines.machines.clone_from(&before.machines);
    if &non_machines != before {
        return Err(RewriteError::ChangedProgramStructure);
    }
    Ok(())
}
