use psi_arena::{Arena, Handle, HandleSpan};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StateBorrowRootKind {
    #[default]
    OwnedData,
    LocalData,
    MutableParameter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowWritableRoot {
    pub symbol: SymbolHandle,
    pub kind: StateBorrowRootKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StateBorrowAccessKind {
    #[default]
    Read,
    Mutable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowArgumentAccess {
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
    pub kind: StateBorrowAccessKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowCall {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<StateBorrowArgumentAccess>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowLoan {
    pub statement_index: usize,
    pub last_use_statement_index: usize,
    pub owner_symbol: SymbolHandle,
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateBorrowEventSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
}

impl Default for StateBorrowEventSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowActivation {
    pub source: StateBorrowEventSource,
    pub loan: Handle<StateBorrowLoan>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StateBorrowWeakeningReason {
    #[default]
    LastUseExpired,
    StateExit,
    LocalReassigned,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowWeakening {
    pub source: StateBorrowEventSource,
    pub loan: Handle<StateBorrowLoan>,
    pub reason: StateBorrowWeakeningReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowSummary {
    pub writable_roots: HandleSpan<StateBorrowWritableRoot>,
    pub mutable_parameter_count: usize,
    pub calls: HandleSpan<StateBorrowCall>,
    pub active_loans: HandleSpan<StateBorrowLoan>,
    pub activations: HandleSpan<StateBorrowActivation>,
    pub weakenings: HandleSpan<StateBorrowWeakening>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowBorrowRoots {
    pub writable_roots: Arena<StateBorrowWritableRoot>,
    pub access_segments: Arena<psi_facts::PlaceSegment>,
    pub argument_accesses: Arena<StateBorrowArgumentAccess>,
    pub calls: Arena<StateBorrowCall>,
    pub loans: Arena<StateBorrowLoan>,
    pub activations: Arena<StateBorrowActivation>,
    pub weakenings: Arena<StateBorrowWeakening>,
}

impl ControlFlowBorrowRoots {
    pub fn with_roots(
        writable_roots: Arena<StateBorrowWritableRoot>,
        access_segments: Arena<psi_facts::PlaceSegment>,
        argument_accesses: Arena<StateBorrowArgumentAccess>,
        calls: Arena<StateBorrowCall>,
        loans: Arena<StateBorrowLoan>,
        activations: Arena<StateBorrowActivation>,
        weakenings: Arena<StateBorrowWeakening>,
    ) -> Self {
        Self {
            writable_roots,
            access_segments,
            argument_accesses,
            calls,
            loans,
            activations,
            weakenings,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ControlFlowBorrowRoots, StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall,
        StateBorrowLoan, StateBorrowWeakening, StateBorrowWritableRoot,
    };
    use psi_arena::Arena;

    #[test]
    fn borrow_roots_constructor_keeps_borrow_noun_roots_explicit() {
        let writable_roots = Arena::<StateBorrowWritableRoot>::with_capacity(1);
        let access_segments = Arena::<psi_facts::PlaceSegment>::with_capacity(2);
        let argument_accesses = Arena::<StateBorrowArgumentAccess>::with_capacity(3);
        let calls = Arena::<StateBorrowCall>::with_capacity(4);
        let loans = Arena::<StateBorrowLoan>::with_capacity(5);
        let activations = Arena::<StateBorrowActivation>::with_capacity(6);
        let weakenings = Arena::<StateBorrowWeakening>::with_capacity(7);

        let roots = ControlFlowBorrowRoots::with_roots(
            writable_roots.clone(),
            access_segments.clone(),
            argument_accesses.clone(),
            calls.clone(),
            loans.clone(),
            activations.clone(),
            weakenings.clone(),
        );

        assert_eq!(roots.writable_roots, writable_roots);
        assert_eq!(roots.access_segments, access_segments);
        assert_eq!(roots.argument_accesses, argument_accesses);
        assert_eq!(roots.calls, calls);
        assert_eq!(roots.loans, loans);
        assert_eq!(roots.activations, activations);
        assert_eq!(roots.weakenings, weakenings);
    }
}
