//! Private, bit-preserving runtime-value storage. Every admitted use reads its
//! own storage through a reload pair; when admission proves some physical view
//! of the victim's class could host an interval at all — no implicitly used,
//! pinned, or reserved unit occupies every view — the first unpinned
//! instruction-operand use in a block emits the pair and every later unpinned
//! use names that same still-open reload register. How far that pair reaches is
//! the span policy: under `UnitWriteBounded` an instruction that can destroy
//! register content — a clobber or an implicit definition — closes the open
//! pair, so the next unpinned use opens a fresh one and no produced interval
//! ever reaches across a call. Under `UnitWriteCrossing` the open pair instead
//! survives such an instruction while an allocatable view of the victim's class
//! avoids every unit written inside the span — exactly the clobber-set-reduced,
//! most often callee-saved, home the produced interval then demands of the
//! allocator. ABI-pinned uses keep a private pair so the pin attaches
//! to the load-to-use window alone, and blocks with no surviving view keep the
//! per-use shape entirely.
//! Admission and control projection are shared predicates; proposal inserts the
//! private accesses while independent replay consumes them and restores source.
//!
//! Instruction-result definitions must dominate every flexible use. Incoming parameters
//! instead require a dedicated edge definition on every predecessor: the edge
//! copy's own output register, or — on a case-dispatch continuation — the
//! bridge's field observation behind the payload's own `Load32`/`Load64`.
//! Each is stored immediately, before later transfers create more pressure.
//! A later `Def` operand on the victim redefines the register, so the slot
//! tracks it the same way: one store after every definition keeps the slot a
//! mirror of the register's last write, a use on the redefining instruction
//! still reads the pre-write value through its own reload, and the
//! redefinition closes any still-open shared reload since the value it held
//! no longer restates the register. The origin definition still dominates
//! every use, so on each path the last executed store is the register's
//! reaching write.
//! Original parameter bindings remain exact. Replacing the destination's uses
//! makes that parameter dead, so fresh liveness no longer requires its edge
//! home tie. Their destination must likewise dominate every use.
//! An entry-bound register's definition is the function-entry boundary
//! itself: a scalar entry parameter, a structural parameter's incoming
//! pointer, or the hidden aggregate-result destination each arrives live-in,
//! so its single store opens the entry block and its ABI live-in view keeps
//! pinning only the entry-to-store window. A structural live-in's provenance
//! is the contract's parameter row or declared result place rather than a
//! source site — it restates no `ValueId`, so its reload registers carry a
//! structural-observation origin and no value binding may name it. That
//! boundary store stays admitted only while no edge targets the entry block —
//! re-entry would re-read a register whose interval already ended at the
//! first store.
//! Instruction-defined structural registers follow the result definition
//! rule: a field observation's load result, a retained transport pointer
//! copy, or a snapshot chunk word carries a declared place and byte offset
//! rather than a `ValueId`, so its store lands after the defining instruction
//! and its reload registers carry that same coordinate in their observation
//! origin — exact enough that a reload can still serve a case-payload
//! argument whose transport demands the declared field. The definition's own
//! instruction may be an address producer — `FrameAddress`, `AddressOffset`,
//! or `ByteViewAddress` — since a resolved address round-trips its bits like
//! any other result; the single exception is the primitive-local
//! establishment idiom, where the `WritePlace` store paired with the
//! `AddressLocal` access on that definition keeps the victim register in its
//! address operand verbatim, and the coordinate stays rejected where a
//! transport names a different place or field offset.
//!
//! A victim whose class cannot sit in the slot's access rows — an
//! ABI-resident IEEE-float register on a target whose frame transport is
//! GPR — still spills: its stored bytes are raw payload, so each store is
//! the target's own `Float*ToBits` conversion into the rows' shared carrier
//! class followed by the `Store64`, and each reload appends the matching
//! `BitsToFloat*` behind its `Load64`. The eight-byte private slot, the
//! slot-sharing analysis, and the boundary/redefinition ordering are
//! unchanged; the conversions are inert two-operand bridges — a row with an
//! implicit definition or clobber could destroy the still-open reload it
//! sits inside, so an impure or missing pair leaves the victim a
//! candidate-local rejection rather than a constraint fault.
//!
//! Terminator instruction operands are ordinary uses at one more position:
//! each reloads at the end of its block, after the last body instruction and
//! any definition store. A fixed view on such an operand (an ABI return or
//! exit register) stays on the rewritten operand, pinning the fresh reload
//! register to the same physical unit. The same holds for a fixed view on a
//! body instruction operand (an ABI call argument or result site): its reload
//! pair is inserted immediately before the consumer, so the precolored
//! segment covers exactly the load-to-use window. A `Registers` value-binding argument
//! on an outgoing successor reads at that same end-of-block position, after
//! the terminator instruction executes: its reload pair follows any
//! terminator-operand pairs in successor then binding order, and the binding
//! keeps its semantic declaration while moving to the fresh reload register.
//! A `Registers` case-payload argument reads at that same position: its pairs
//! follow the edge's binding pairs in payload order, and the payload's
//! declared type must equal the victim's exact type. The parameter side of a
//! binding or payload is not a use at all — it is the destination's
//! definition, admitted only when it is the block-parameter victim's own
//! incoming edge; every other naming stays rejected.
//!
//! A stored structural transport — a `Descriptor` or `WholeValue` snapshot
//! argument on an edge-transfer continuation — reads the victim only through
//! the bridge's own snapshot chunk loads, which admission treats as ordinary
//! body instruction uses: each reload pair sits before its load, inside the
//! bridge body rather than at the end-of-block position. The binding's
//! `argument` field then moves to the single register those loads name after
//! rewriting — no pair of its own is emitted. Admission requires the exact
//! chunk-load stream the transport's byte decomposition describes, recorded
//! as `ReadPlace` accesses for the edge and source place in this block. A
//! one-chunk snapshot always resolves to one register; a multi-chunk
//! snapshot needs the block's shared open reload, so it is admitted only
//! where every chunk operand is unpinned and no unit-writing instruction
//! closes the span between the first and last chunk load. A stored argument
//! on the victim's own incoming edge, on a non-continuation successor, or
//! whose chunk loads cannot resolve to one register stays rejected.
//! Cyclic functions stay admitted: a back edge
//! reaching the destination is just one more incoming edge, and it must run
//! the same dedicated edge-copy definition whose store initializes the slot
//! before the destination — and therefore every dominated use — executes.
//!
//! Each retained rewrite shares unchanged selected functions. Replay still
//! restores and compares the complete source by content, so separately allocated
//! equivalent inputs work and corruption of an unrelated function rejects.
//!
//! A victim also shares an already-declared spill slot when the candidate's
//! every access follows this rewrite's own idiom — a zero-offset `Store64`, or
//! a zero-offset `FrameAddress` feeding only zero-offset `Load64`s — and a
//! last-writer replay over the function proves the incumbent's windows and the
//! new victim's cannot interleave: no incumbent load may observe a new store,
//! and every new reload must be reached by new stores alone. Sharing appends
//! no `local_storage_slots` entry, so the eight bytes stay charged to the frame
//! exactly once; any other naming of the candidate, an escaped address, or an
//! interleaved window falls back to a private slot.

mod admission;
mod rewrite;
mod slot;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::{spill_selected_runtime_value, spill_selected_runtime_value_with_span_policy};
pub use validation::{validate_runtime_spill, validate_runtime_spill_with_span_policy};

/// How far a block's still-open shared reload interval may reach. Admission
/// fixes this once per rewrite so proposal and independent replay share one
/// decision procedure over the same recorded unit writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSpillSpanPolicy {
    /// A clobber or implicit definition always closes the open pair: every
    /// produced interval stays inside a unit-free span and never demands a
    /// call-surviving home.
    UnitWriteBounded,
    /// A unit-writing instruction — most often a `CallUnit` — keeps the open
    /// pair while an allocatable view of the victim's class avoids every unit
    /// written inside the span so far: the surviving homes a call crossing
    /// demands. When no such view exists the instruction still closes the
    /// span, so the produced shape degrades to the bounded one.
    UnitWriteCrossing,
}

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
/// Shared with the runtime-rematerialization recovery rewrite.
pub(crate) fn control(
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

/// The same projection for operand substitution. Successor edges and their
/// transports stay untouched; only the instruction's operand registers change.
/// Shared with the runtime-rematerialization recovery rewrite.
pub(crate) fn control_mut(
    terminator: &mut selected_instructions::SelectedTerminator,
) -> &mut selected_instructions::SelectedInstruction {
    use selected_instructions::SelectedTerminator;
    match terminator {
        SelectedTerminator::Return { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. } => instruction,
    }
}

/// The same projection for successor transports. The terminator instruction is
/// untouched; only the outgoing successors' value bindings can move to fresh
/// reload registers. Order matches `control`: polarity zero before one.
pub(crate) fn control_successors_mut(
    terminator: &mut selected_instructions::SelectedTerminator,
) -> [Option<&mut selected_instructions::SelectedSuccessor>; 2] {
    use selected_instructions::SelectedTerminator;
    match terminator {
        SelectedTerminator::Return { .. } | SelectedTerminator::HostedExitProcess { .. } => {
            [None, None]
        }
        SelectedTerminator::Jump { successor, .. } => [Some(successor), None],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => [Some(when_nonzero), Some(when_zero)],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => [Some(when_less), Some(when_not_less)],
    }
}

/// Removing the initialization anchor must disconnect every use from entry.
/// Together with ordinary reachability, this proves dominance without assuming
/// that the block vector is topological or that a use is an edge-copy idiom.
/// Shared with the runtime-rematerialization recovery rewrite.
pub(crate) fn require_dominated_uses(
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
