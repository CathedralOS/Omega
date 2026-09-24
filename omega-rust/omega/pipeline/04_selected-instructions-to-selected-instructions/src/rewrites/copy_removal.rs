//! Copy removal on the selected CFG.
//!
//! A `CopyI64` duplicates one register's bit pattern into another. When every
//! reader of the destination register sits later in the same block — an
//! ordinary body operand or one carried by the block's terminator — each of
//! those readers can instead read the copied source directly, provided the
//! source still holds the value the copy captured. The transformation rebinds
//! every admitted use operand to the source register, removes the copy
//! instruction, and drops the destination register's roster row: the copied
//! value was never materialized under another identity, so nothing else may
//! keep naming the destination.
//!
//! The contract is deliberately narrow. The destination's only definition in
//! the function must be this copy — a `UseDef` rewrite, an edge `parameter`,
//! or a second definition operand refuses — and its roster origin must claim
//! this copy (`InstructionResult` or `InstructionScratch` of it), because a
//! register whose origin names another surface keeps a definition this
//! removal cannot honor. Substitution is same-block only: uses through
//! successor bindings, structural transports, or case payloads read the
//! register on an edge and reject, as do `Spill` storage slots, other
//! registers' `SpillAddress` origins, memory-access rows, and call
//! contracts — each would lose its referent when the register row disappears.
//!
//! Same-block substitution is sound because a block executes in order: the
//! destination's value at each admitted use equals the source's value at the
//! copy, and every instruction strictly between the copy and the last use is
//! scanned for a redefinition of the source (`Def` or `UseDef`), which
//! refuses. A redefinition carried by the use instruction itself is still
//! safe — operand uses read pre-definition — so the scanned interval stops
//! before the last use's own instruction, terminator-carried or not.
//!
//! Removing the copy shortens the block's instruction vector, so boundary
//! settlements positioned after it shift one ordinal earlier; positions at
//! or before it are untouched. Settlements carry no register references, so
//! the substitution itself is invisible to them. Removing the destination's
//! roster row and the copy instruction also re-densifies the function's
//! identifiers — readers index `virtual_registers` by register id — so every
//! register id above the destination and every instruction id above the copy
//! shifts down one on instruction ids, operands, call and access rows,
//! roster origins, storage slots, and edge transports.
//!
//! Proposal and validation decide legality independently. The producer's
//! `admission` locates the copy, classifies the destination's mentions, and
//! applies the substitution; `validation` re-derives the same contract from
//! the source records on its own audit — sharing only the representation
//! walkers in `block_edges` — then requires the proposal to equal the
//! function its own record produces and restores the complete source by
//! content — every other instruction, register, roster row, call, and
//! settlement included. A wrong admission decision fails validation even
//! when the proposal is exactly the producer's edit.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{
    CopyRemovalIdentity, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use rewrite::remove_selected_copy;
pub(crate) use validation::{measured_steps, validate_copy_removal};

#[cfg(test)]
mod tests;

/// An accepted copy removal with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCopyRemoval {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: CopyRemovalReceipt,
}

impl ValidatedCopyRemoval {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &CopyRemovalReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyRemovalReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    function_index: usize,
    copy: SelectedInstructionId,
}

impl CopyRemovalReceipt {
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
    /// The function the copy was removed from.
    pub const fn function_index(&self) -> usize {
        self.function_index
    }
    /// The removed `CopyI64` instruction's source-side identity.
    pub const fn copy(&self) -> SelectedInstructionId {
        self.copy
    }
    /// The durable transformation identity the post-allocation manifest
    /// ledger records: the receipt's exact fields under the copy-removal
    /// domain separator.
    pub fn identity(&self) -> CopyRemovalIdentity {
        copy_removal_identity(self)
    }
}

/// Canonical identity of one validated copy removal: every receipt field —
/// the coordinate, both plan identities, and the proof inputs — is part of
/// the durable record, so two removals of the same instruction under
/// different sources stay distinct transformations.
pub(crate) fn copy_removal_identity(receipt: &CopyRemovalReceipt) -> CopyRemovalIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-copy-removal.v1\0");
    bytes.extend_from_slice(&receipt.source_selected.bytes());
    bytes.extend_from_slice(&receipt.transformed_selected.bytes());
    bytes.extend_from_slice(&receipt.optimization_unit.bytes());
    bytes.extend_from_slice(&receipt.fuel_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(
        &u64::try_from(receipt.function_index)
            .expect("copy-removal function index fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&receipt.copy.0.to_le_bytes());
    CopyRemovalIdentity::from_canonical_bytes(&bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyRemovalError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedRegister,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for CopyRemovalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid copy removal: {self:?}")
    }
}

impl std::error::Error for CopyRemovalError {}
