//! Address folding on the selected CFG.
//!
//! Selection emits a projected referent address as `AddressOffset`
//! (`result = base + byte_offset`), and ordinary loads, referent stores, and
//! further address projections read that result through their own base
//! operand while carrying a second displacement in the instruction kind.
//! Between the producer and such a consumer the machine performs two adds
//! for one effective address: `pointer = base + k`, then `[pointer + m]`.
//! When the pointer operand's in-block producer is exactly one
//! `AddressOffset` whose own base still holds the value the producer read,
//! the consumer can read that base directly and carry the combined
//! displacement `k + m` — the accessed bytes are identical.
//!
//! This rewrite is that address folding at full selected-CFG standing. The
//! consumer keeps its instruction identity, position, provenance, result or
//! value operand, implicit surfaces, and constraint row; only its kind's
//! displacement and its operand-0 register change. The `AddressOffset`
//! producer is retained — its register may have other readers — so producer
//! elimination stays with the producer-elimination rules, not this one.
//!
//! The admission table is deliberately narrow. Only operand 0 (the base
//! pointer `Use`) can fold: it is the operand the encoder's `base_operand: 0`
//! address forms read. The producer must be the last definition of the
//! pointer register before the consumer in the same block — a `UseDef`
//! rewrite or another instruction defining it later refuses — and nothing
//! strictly between the producer and the consumer may redefine the producer's
//! own base register, so the base the consumer reads is the value the
//! producer offset. A definition carried by the consumer itself is still
//! safe: operand uses read pre-definition.
//!
//! The combined displacement must fit the bound every target encoder shares
//! for the consumer's form. AArch64 encodes these addressing forms as a
//! scaled unsigned immediate (`displacement` a multiple of the access width,
//! `displacement / width <= 4095`); x86-64 admits a wider signed disp32, so
//! the shared rule uses the narrower AArch64 bound for every target — the
//! same target-independent bound the selected-lowering `u12` pair rules
//! declare. `Load8` and `AddressOffset` scale by one; `Load16`, `Load32`,
//! `Load64`, and `Store` scale by their byte width. A combined offset that
//! is unaligned or out of range refuses rather than emitting an unencodable
//! form.
//!
//! The memory-access roster is untouched: it records each access's semantic
//! place, byte offset, byte count, and role, and folding a referent offset
//! into the displacement moves no bytes — the same place range is observed.
//! Calls, boundary settlements, transports, and every other function,
//! register, and instruction are retained bit-identical.
//!
//! Proposal and independent replay share only the small leaf predicates
//! (`consumer_shape`, `admitted_offset`, `last_definition_before`).
//! Validation locates the one changed instruction by comparing the proposal
//! to the source, audits that position's fold legality itself — never
//! consulting the producer's admission — requires the proposed instruction
//! to equal the independently derived folded form, and restores the complete
//! source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::fold_selected_address;
pub use validation::validate_address_fold;

#[cfg(test)]
mod tests;

/// An accepted address fold with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAddressFold {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: AddressFoldReceipt,
}

impl ValidatedAddressFold {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &AddressFoldReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressFoldReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl AddressFoldReceipt {
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
pub enum AddressFoldError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedProducer,
    UnsupportedOffset,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for AddressFoldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid address fold: {self:?}")
    }
}

impl std::error::Error for AddressFoldError {}
