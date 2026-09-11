//! Private, bit-preserving runtime-value storage. Each use gets its own reload;
//! no reload interval needs to survive an intervening call.
//! Admission and control projection are shared predicates; proposal inserts the
//! private accesses while independent replay consumes them and restores source.
//!
//! Instruction-result definitions must dominate every flexible use. Incoming parameters
//! instead require a dedicated edge-copy definition on every predecessor: each
//! copy is stored immediately, before later transfers create more pressure.
//! Original parameter bindings remain exact. Replacing the destination's uses
//! makes that parameter dead, so fresh liveness no longer requires its edge
//! home tie. Their destination must likewise dominate every use. Terminator
//! operands, outgoing value transports, and cyclic functions remain unsupported.
//!
//! Each retained rewrite shares unchanged selected functions. Replay still
//! restores and compares the complete source by content, so separately allocated
//! equivalent inputs work and corruption of an unrelated function rejects.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::spill_selected_runtime_value;
pub use validation::validate_runtime_spill;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuntimeSpill {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: RuntimeSpillReceipt,
}

impl ValidatedRuntimeSpill {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &RuntimeSpillReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSpillReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl RuntimeSpillReceipt {
    pub const fn source_selected(&self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn transformed_selected(&self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn optimization_unit(&self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(&self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSpillError {
    SourceMismatch,
    UnsupportedValue,
    UnsupportedControlFlow,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for RuntimeSpillError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid runtime-value spill: {self:?}")
    }
}

impl std::error::Error for RuntimeSpillError {}

/// Project every selected terminator without conflating edge transport with
/// instruction operands. Both can keep a virtual value live across blocks.
fn control(
    terminator: &selected_instructions::SelectedTerminator,
) -> (
    &selected_instructions::SelectedInstruction,
    [Option<&selected_instructions::SelectedSuccessor>; 2],
) {
    use selected_instructions::SelectedTerminator;
    match terminator {
        SelectedTerminator::Return { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. } => (instruction, [None, None]),
        SelectedTerminator::Jump {
            instruction,
            successor,
        } => (instruction, [Some(successor), None]),
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => (instruction, [Some(when_nonzero), Some(when_zero)]),
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (instruction, [Some(when_less), Some(when_not_less)]),
    }
}

/// Cyclic functions retain their frozen-source contract, including prefixes and
/// exits. Walk all components; block vector order is not execution order.
fn require_acyclic(
    function: &selected_instructions::SelectedFunction,
) -> Result<(), RuntimeSpillError> {
    let mut colors = vec![0u8; function.blocks.len()];
    let mut pending = Vec::new();
    for root in 0..function.blocks.len() {
        if colors[root] != 0 {
            continue;
        }
        colors[root] = 1;
        pending.push((root, 0usize));
        while let Some((block_index, successor_index)) = pending.last_mut() {
            if *successor_index == 2 {
                colors[*block_index] = 2;
                pending.pop();
                continue;
            }
            let successor = control(&function.blocks[*block_index].terminator).1[*successor_index];
            *successor_index += 1;
            let Some(successor) = successor else { continue };
            let destination = function
                .blocks
                .iter()
                .position(|block| block.id == successor.block)
                .ok_or(RuntimeSpillError::SourceMismatch)?;
            match colors[destination] {
                1 => return Err(RuntimeSpillError::UnsupportedControlFlow),
                0 => {
                    colors[destination] = 1;
                    pending.push((destination, 0));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Removing the initialization anchor must disconnect every use from entry.
/// Together with ordinary reachability, this proves dominance without assuming
/// that the block vector is topological or that a use is an edge-copy idiom.
fn require_dominated_uses(
    function: &selected_instructions::SelectedFunction,
    anchor: usize,
    use_blocks: &[usize],
) -> Result<(), RuntimeSpillError> {
    let entry = function
        .blocks
        .iter()
        .position(|block| block.id == function.entry_block)
        .ok_or(RuntimeSpillError::SourceMismatch)?;
    for bypass_anchor in [false, true] {
        let mut reached = vec![false; function.blocks.len()];
        let mut pending = vec![entry];
        while let Some(block_index) = pending.pop() {
            if reached[block_index] || (bypass_anchor && block_index == anchor) {
                continue;
            }
            reached[block_index] = true;
            for successor in control(&function.blocks[block_index].terminator)
                .1
                .into_iter()
                .flatten()
            {
                let destination = function
                    .blocks
                    .iter()
                    .position(|block| block.id == successor.block)
                    .ok_or(RuntimeSpillError::SourceMismatch)?;
                pending.push(destination);
            }
        }
        if (!bypass_anchor && !reached[anchor])
            || use_blocks
                .iter()
                .any(|block_index| reached[*block_index] == bypass_anchor)
        {
            return Err(RuntimeSpillError::UnsupportedUse);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
