//! Whole-provider choice over already-owned declaration descriptions.
//!
//! The same decision serves source dependency discovery and native plan
//! selection. Adapters own exact declaration, coverage and receiver-contract
//! correspondence. Choosing a description grants no checking, ABI, supply or
//! installation authority; those remain obligations of the receiving owner.
//!
//! A slot is settled by the first of: an explicit override, a target default,
//! a covering candidate that a Build row keys to the selected target, or the
//! one covering candidate in scope. Candidates in scope for every target stay
//! checked but do not compete with a row that names this target.

mod identities;
pub use identities::{ApplicationKey, selected_application_slots};
mod selection;
pub use selection::choose;

/// One complete candidate, never an independently selectable method row.
pub trait Candidate {
    type Slot: Clone + Eq + Ord;
    type Provider: Clone + Eq + Ord;

    fn slot(&self) -> &Self::Slot;
    fn provider(&self) -> &Self::Provider;
    fn target(&self) -> &str;
    /// Whether the candidate exists because an authored Build row names the
    /// selected target (a native binding), rather than because an
    /// implementation is merely in scope for every target.
    fn build_keyed(&self) -> bool;
    fn has_requirements(&self) -> bool;
    fn covers_requirements(&self) -> bool;
    fn receiver_count(&self) -> usize;
    fn supplies_receiver(&self, demanded: &Self) -> Option<bool>;
    fn shares_receiver(&self, other: &Self) -> Option<bool>;
}

/// An exact authored choice. Bare declarations may cover several prepared
/// applications; an applied choice retains its complete independent telescope.
pub trait Declaration<Slot, Provider> {
    fn target(&self) -> Option<&str>;
    fn slots(&self, available: &[Slot]) -> Vec<Slot>;
    fn provider(&self) -> &Provider;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Override,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    Override(Vec<usize>),
    Default(Vec<usize>),
    UniqueCoveringCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub candidate: usize,
    pub reason: Reason,
}

/// Coordinates always refer to the borrowed input rosters supplied to choose.
/// Diagnostic names and any provider-specific receipts stay with that adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error<Slot, Provider> {
    UnknownSlot {
        tier: Tier,
        declaration: usize,
        slot: Slot,
    },
    RepeatedOverride {
        slot: Slot,
        declarations: Vec<usize>,
    },
    ConflictingDefaults {
        slot: Slot,
        providers: Vec<Provider>,
    },
    PartialChoice {
        tier: Tier,
        declaration: usize,
        candidate: usize,
    },
    MissingChoice {
        tier: Tier,
        declaration: usize,
        slot: Slot,
        wrong_target: bool,
    },
    RepeatedCandidate {
        tier: Tier,
        declaration: usize,
        slot: Slot,
    },
    UnresolvedInheritedReceiver {
        slot: Slot,
    },
    UnresolvedOverlap {
        first: usize,
        second: usize,
    },
    RepeatedInheritedReceiver {
        slot: Slot,
    },
    AmbiguousCandidates {
        slot: Slot,
        candidates: Vec<usize>,
    },
    OverlappingChoices {
        first: usize,
        second: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome<Slot, Provider> {
    /// Successful independent slots remain descriptions even if another slot
    /// is unresolved. An admitting consumer must require errors to be empty.
    pub choices: Vec<Choice>,
    pub errors: Vec<Error<Slot, Provider>>,
}

#[cfg(test)]
mod tests;
