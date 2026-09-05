//! Independent checks of target-neutral rewrites before Terminal publication.

use crate::{
    ModuleError, reconstruct_optimizable_terminal_obligations, validate_module_for_optimization,
};
use semantic_vocabulary::OperationId;
use terminal_psi::TerminalModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadScalarRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedSurvivingOperation(OperationId),
    RemovedNonTotalOperation(OperationId),
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
    for ((old, new), restored_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut restored.machines)
    {
        if old.blocks.len() != new.blocks.len() {
            return Err(DeadScalarRewriteError::ChangedProgramStructure);
        }
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
            restored_block.operations.clone_from(&old_block.operations);
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
