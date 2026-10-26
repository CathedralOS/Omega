//! Borrow-aware store-to-load forwarding on the selected CFG.
//!
//! A `Store { byte_offset, byte_size }` writes the low exact-width bits of
//! its value operand through the referent pointer — or, carrying a
//! `WriteLocal` row, through the materialized address of the place's own
//! storage slot, which is the place's storage under the same byte
//! coordinates: its `StructuralParameter`/`StructuralBlockParameter` home,
//! or the `Structural` slot of the operation the place's declaration names
//! as its producer. A `Store64` into that
//! slot writes the same bytes directly. A later `Load64`/`Load32`/
//! `Load16`/`Load8` of the same width at the same byte offset observes
//! exactly those bytes when no intervening instruction can write or expose
//! the same storage. The rewrite replaces the load with a
//! `CopyI64` of the stored register at full width, or the matching
//! `ZeroExtend` at sub-word width: a same-width store-then-load round-trips
//! the register's low bits in the target's own byte order, so no endianness
//! assumption is needed. The loaded register keeps its identity, origin, and
//! the read's provenance.
//!
//! A `Load8Indexed` reads a dynamic extent instead — the single byte at its
//! `ReadByteSequence` row's payload base plus the runtime index the row
//! carries — so only the byte-exact writer sources the forward: a
//! `Store { 0, 1 }` through a fully computed view address whose
//! `WriteByteSequence` row names the same payload base and the same index
//! value. The replacement is `ZeroExtendU8` of the stored register. An
//! exact or local range cannot contain a runtime-placed byte, and a
//! sequence write at another offset or index may land on a different byte
//! entirely, so neither can serve as the source.
//!
//! The store and the load need not share a block: when a block's top is
//! reached without interference, the walk crosses into every predecessor
//! block, and the block resolves once each predecessor path's last writer
//! stored the same register — one store dominating the join, or each leg's
//! own store of that register. Legs that stored different registers still
//! resolve when the block's own parameter already merges them: a block
//! parameter every incoming edge binds from exactly that leg's register,
//! so the parameter itself is the carried value. A deferred block writes
//! nothing in its
//! walked span, so a self-loop or writerless cycle still resolves when
//! every leg arriving into it carries that register, provided the load's
//! own block on the cycle shows a clear tail behind the load. The function
//! entry, a block no edge reaches, and a deferred region whose arriving
//! legs disagree without an edge merge or never settle each leave the walk
//! without a pair.
//! Crossed edges are admitted only when their transports move neither the
//! carried registers nor storage the forwarded place can reach.
//!
//! Distinct place identities do not establish byte disjointness. Until a
//! checked physical-alias bridge is retained, crossing another place's write
//! conservatively refuses. Exact disjoint extents of the same captured place
//! and compiler-private staging storage remain admissible.
//!
//! Alias legality is reconstructed from the validated memory roster, never
//! pointer-register equality. Exact reads and writes on the same captured place
//! compare byte extents. Writes or exposed storage under another place identity
//! refuse unless their route is compiler-private staging; exclusivity does not
//! supply physical disjointness. Dynamic extents retain their lower-bounded
//! reach, and unknown memory effects and calls block the walk.
//!
//! Instructions inserted by private-slot rewrites (spill stores, reloads,
//! frame addresses over `Spill`/`Boundary` slots) carry no roster row; they
//! touch compiler-owned activation storage that no referent place aliases.
//! Every other memory-capable instruction without a row, every call, and every
//! hosted effect conservatively blocks forwarding.
//!
//! Proposal and validation share only the contract: the validator never
//! calls the producer's admission. It re-derives the pair's legality from
//! the source records — the load's roster row, the backward walk, the
//! register and edge audits, the target's copy row — rebuilds the demanded
//! function itself, requires the proposed program to equal it, and restores
//! the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    StoredLoadForwardingIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use rewrite::forward_selected_stored_load;

/// An accepted store-to-load forwarding with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedStoredLoadForwarding {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: StoredLoadForwardingReceipt,
}

validated_transformed_receipt!(ValidatedStoredLoadForwarding, StoredLoadForwardingReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLoadForwardingReceipt {
    identity: StoredLoadForwardingIdentity,
    function_index: usize,
    load: SelectedInstructionId,
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl StoredLoadForwardingReceipt {
    receipt_accessor_rows!(ref StoredLoadForwardingReceipt {
        identity: StoredLoadForwardingIdentity,
        function_index: usize,
        load: SelectedInstructionId,
    });
}

receipt_selected_identities!(StoredLoadForwardingReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredLoadForwardingError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedPair,
    UnsupportedUse,
    AliasingWrite,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for StoredLoadForwardingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid stored-load forwarding: {self:?}")
    }
}

impl std::error::Error for StoredLoadForwardingError {}
