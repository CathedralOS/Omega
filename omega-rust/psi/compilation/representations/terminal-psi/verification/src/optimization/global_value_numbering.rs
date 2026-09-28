//! Validation of global value numbering rewrites.

use crate::{
    ModuleError, reconstruct_optimizable_crash_obligations,
    reconstruct_optimizable_terminal_obligations, validate_module_for_optimization,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, ValueId};
use std::collections::BTreeMap;
use terminal_psi::TerminalModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalValueNumberingRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    RemovedIneligibleOperation(OperationId),
    MissingDominatingEquivalent(OperationId),
    ChangedMachine(MachineId),
    ChangedProofQuestion,
}

/// Check the exact duplicate-removal relation, not the producer's numbering.
///
/// A removed operation is justified only when a surviving
/// unconditionally-total scalar operation with the same kind and an equal
/// result declaration dominates it: earlier in its own block or anywhere in a
/// dominating block. Operand identities compare after resolving the results
/// of already-removed duplicates to their canonical survivors, so the check
/// re-derives which surviving result each removed operation must map to,
/// applies exactly that substitution at every direct scalar use, and compares
/// the reconstructed module to `after`. The canonical survivor is the first
/// matching operation in reverse postorder, so the producer cannot pick a
/// different representative and still verify.
pub fn validate_global_value_numbering(
    before: &TerminalModule,
    after: &TerminalModule,
) -> Result<(), GlobalValueNumberingRewriteError> {
    let before_valid = validate_module_for_optimization(before)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    let after_valid = validate_module_for_optimization(after)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    if before.machines.len() != after.machines.len() {
        return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
    }
    let mut expected = before.clone();
    for ((old, new), expected_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut expected.machines)
    {
        if old.blocks.len() != new.blocks.len() {
            return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
        }
        let mut surviving = std::collections::BTreeSet::new();
        for (old_block, new_block) in old.blocks.iter().zip(&new.blocks) {
            if old_block.id != new_block.id
                || old_block.parameters != new_block.parameters
                || old_block.structural_parameters != new_block.structural_parameters
            {
                return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
            }
            // Surviving operations keep their identity and relative order;
            // substituted operand content is checked against `expected` below.
            let mut retained = old_block.operations.iter();
            for operation in &new_block.operations {
                if !retained
                    .by_ref()
                    .any(|old_operation| old_operation.id == operation.id)
                {
                    return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
                }
                surviving.insert(operation.id);
            }
        }
        let dominators = crate::control_graph::dominators(old);
        let mut representative: BTreeMap<ValueId, ValueId> = BTreeMap::new();
        let mut leaders: Vec<(
            terminal_psi::OperationKind,
            BlockId,
            terminal_psi::ValueDeclaration,
        )> = Vec::new();
        for block_id in crate::control_graph::reverse_postorder(old) {
            let old_block = old
                .blocks
                .iter()
                .find(|block| block.id == block_id)
                .ok_or(GlobalValueNumberingRewriteError::ChangedProgramStructure)?;
            for operation in &old_block.operations {
                let eligible = operation.static_reach_binding.is_none()
                    && operation.result.scalar().is_some()
                    && terminal_semantics::is_unconditionally_total_scalar(&operation.kind);
                if surviving.contains(&operation.id) {
                    if eligible {
                        let mut kind = operation.kind.clone();
                        kind.map_scalar_uses(&mut |value| {
                            representative.get(&value).copied().unwrap_or(value)
                        });
                        leaders.push((kind, block_id, operation.result.expect_scalar()));
                    }
                    continue;
                }
                if !eligible {
                    return Err(
                        GlobalValueNumberingRewriteError::RemovedIneligibleOperation(operation.id),
                    );
                }
                let mut kind = operation.kind.clone();
                kind.map_scalar_uses(&mut |value| {
                    representative.get(&value).copied().unwrap_or(value)
                });
                let result = operation.result.expect_scalar();
                let Some((_, _, survivor)) = leaders.iter().find(|(leader_kind, block, leader)| {
                    *leader_kind == kind
                        && dominators.dominates(*block, block_id)
                        && leader.scalar_type == result.scalar_type
                        && leader.qualifications == result.qualifications
                }) else {
                    return Err(
                        GlobalValueNumberingRewriteError::MissingDominatingEquivalent(operation.id),
                    );
                };
                representative.insert(result.id, survivor.id);
            }
        }
        for expected_block in &mut expected_machine.blocks {
            expected_block
                .operations
                .retain(|operation| surviving.contains(&operation.id));
            for operation in &mut expected_block.operations {
                operation.kind.map_scalar_uses(&mut |value| {
                    representative.get(&value).copied().unwrap_or(value)
                });
            }
            expected_block
                .terminator
                .map_scalar_uses(&mut |value| representative.get(&value).copied().unwrap_or(value));
        }
        if expected_machine != new {
            return Err(GlobalValueNumberingRewriteError::ChangedMachine(old.id));
        }
    }
    if &expected != after {
        return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
    }
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    if old_question != new_question {
        return Err(GlobalValueNumberingRewriteError::ChangedProofQuestion);
    }
    // The crash-obligation roster is a second reconstructed question: a
    // rewrite that leaves every terminal obligation intact may still retarget
    // a crash site's reconstructed paths or a continuation's coverage goal,
    // and the supplied certificates answer only the questions they were
    // produced against.
    let old_crash = reconstruct_optimizable_crash_obligations(before_valid)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    let new_crash = reconstruct_optimizable_crash_obligations(after_valid)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    if old_crash != new_crash {
        return Err(GlobalValueNumberingRewriteError::ChangedProofQuestion);
    }
    Ok(())
}
