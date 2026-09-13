//! Independent checks of target-neutral rewrites before Terminal publication.

use crate::{
    ModuleError, reconstruct_optimizable_terminal_obligations, validate_module_for_optimization,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{TerminalModule, Terminator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadScalarRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedSurvivingOperation(OperationId),
    RemovedNonTotalOperation(OperationId),
    ChangedBlockParameters(BlockId),
    ChangedEdgeArguments(EdgeId),
    ChangedProofQuestion,
}

/// Check an operation-removal subsequence, not the producer's liveness result.
/// Output validation independently rejects surviving uses of a removed value.
/// Exact reconstruction preserves assumptions and their numbering, so every
/// unchanged proof bundle is asked exactly the same questions after the rewrite.
pub fn validate_dead_scalar_elimination(
    before: &TerminalModule,
    after: &TerminalModule,
) -> Result<(), DeadScalarRewriteError> {
    let before_valid =
        validate_module_for_optimization(before).map_err(DeadScalarRewriteError::InvalidModule)?;
    let after_valid =
        validate_module_for_optimization(after).map_err(DeadScalarRewriteError::InvalidModule)?;
    if before.machines.len() != after.machines.len() {
        return Err(DeadScalarRewriteError::ChangedProgramStructure);
    }
    let mut restored = after.clone();
    let mut removed_parameters: BTreeMap<MachineId, BTreeMap<BlockId, Vec<usize>>> =
        BTreeMap::new();
    for ((old, new), restored_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut restored.machines)
    {
        if old.blocks.len() != new.blocks.len() {
            return Err(DeadScalarRewriteError::ChangedProgramStructure);
        }
        let mut machine_removed = BTreeMap::new();
        for ((old_block, new_block), restored_block) in old
            .blocks
            .iter()
            .zip(&new.blocks)
            .zip(&mut restored_machine.blocks)
        {
            let mut retained = new_block.operations.iter().peekable();
            for operation in &old_block.operations {
                if retained.peek().is_some_and(|next| next.id == operation.id) {
                    if retained.next() != Some(operation) {
                        return Err(DeadScalarRewriteError::ChangedSurvivingOperation(
                            operation.id,
                        ));
                    }
                } else if operation.result.scalar().is_none()
                    || !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
                {
                    return Err(DeadScalarRewriteError::RemovedNonTotalOperation(
                        operation.id,
                    ));
                }
            }
            if retained.next().is_some() {
                return Err(DeadScalarRewriteError::ChangedProgramStructure);
            }
            let mut retained_parameters = new_block.parameters.iter().peekable();
            let mut removed = Vec::new();
            for (position, parameter) in old_block.parameters.iter().enumerate() {
                if retained_parameters
                    .peek()
                    .is_some_and(|next| next.id == parameter.id)
                {
                    if retained_parameters.next() != Some(parameter) {
                        return Err(DeadScalarRewriteError::ChangedBlockParameters(old_block.id));
                    }
                } else {
                    removed.push(position);
                }
            }
            if retained_parameters.next().is_some() {
                return Err(DeadScalarRewriteError::ChangedBlockParameters(old_block.id));
            }
            if !removed.is_empty() {
                machine_removed.insert(old_block.id, removed);
            }
            restored_block.operations.clone_from(&old_block.operations);
            restored_block.parameters.clone_from(&old_block.parameters);
        }
        if !machine_removed.is_empty() {
            removed_parameters.insert(old.id, machine_removed);
        }
    }
    for ((old, new), restored_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut restored.machines)
    {
        let machine_removed = removed_parameters.get(&old.id);
        for ((old_block, new_block), restored_block) in old
            .blocks
            .iter()
            .zip(&new.blocks)
            .zip(&mut restored_machine.blocks)
        {
            let mut expected = old_block.terminator.clone();
            match &mut expected {
                Terminator::Jump {
                    target, arguments, ..
                } => drop_removed(
                    arguments,
                    machine_removed.and_then(|removed| removed.get(target)),
                ),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        drop_removed(
                            &mut edge.arguments,
                            machine_removed.and_then(|removed| removed.get(&edge.target)),
                        );
                    }
                }
                _ => {}
            }
            if new_block.terminator != expected {
                let edge = new_block
                    .terminator
                    .edges()
                    .next()
                    .ok_or(DeadScalarRewriteError::ChangedProgramStructure)?;
                return Err(DeadScalarRewriteError::ChangedEdgeArguments(edge));
            }
            restored_block.terminator.clone_from(&old_block.terminator);
        }
    }
    if &restored != before {
        return Err(DeadScalarRewriteError::ChangedProgramStructure);
    }
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(DeadScalarRewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(DeadScalarRewriteError::InvalidModule)?;
    if old_question != new_question {
        return Err(DeadScalarRewriteError::ChangedProofQuestion);
    }
    Ok(())
}

/// Drop the listed old positions from one edge's scalar arguments.
fn drop_removed(arguments: &mut Vec<ValueId>, removed: Option<&Vec<usize>>) {
    let Some(removed) = removed else { return };
    let mut position = 0;
    arguments.retain(|_| {
        let retained = !removed.contains(&position);
        position += 1;
        retained
    });
}
