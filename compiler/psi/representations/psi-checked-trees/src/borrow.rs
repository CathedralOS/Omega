use psi_arena::{Arena, HandleSpan};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowRootKind {
    #[default]
    OwnedData,
    LocalData,
    MutableParameter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowWritableRootFact {
    pub symbol: SymbolHandle,
    pub kind: BorrowRootKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub writable_roots: HandleSpan<BorrowWritableRootFact>,
    pub mutable_parameter_count: usize,
    pub calls: HandleSpan<BorrowCallFact>,
    pub loans: HandleSpan<BorrowLoanFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowAccessKind {
    #[default]
    Read,
    Mutable,
    WriteOnly,
}

impl BorrowAccessKind {
    pub fn is_exclusive(&self) -> bool {
        matches!(self, Self::Mutable | Self::WriteOnly)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowArgumentAccessFact {
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
    pub kind: BorrowAccessKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<BorrowArgumentAccessFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowLoanFact {
    pub statement_index: usize,
    pub last_use_statement_index: usize,
    pub owner_symbol: SymbolHandle,
    /// Projection within the owner that carries this loan. An empty path means
    /// the whole owner; dynamic indexes conservatively overlap every element.
    pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
    pub source_owner_symbol: SymbolHandle,
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
    pub kind: BorrowAccessKind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BorrowLoanOwnerSegment {
    Field(SymbolHandle),
    Case(SymbolHandle),
    FixedIndex(usize),
    #[default]
    DynamicIndex,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowFacts {
    pub writable_roots: Arena<BorrowWritableRootFact>,
    pub access_segments: Arena<psi_facts::PlaceSegment>,
    pub owner_segments: Arena<BorrowLoanOwnerSegment>,
    pub argument_accesses: Arena<BorrowArgumentAccessFact>,
    pub calls: Arena<BorrowCallFact>,
    pub loans: Arena<BorrowLoanFact>,
    pub states: Arena<StateBorrowFact>,
}

impl BorrowFacts {
    pub fn with_roots(
        writable_roots: Arena<BorrowWritableRootFact>,
        access_segments: Arena<psi_facts::PlaceSegment>,
        owner_segments: Arena<BorrowLoanOwnerSegment>,
        argument_accesses: Arena<BorrowArgumentAccessFact>,
        calls: Arena<BorrowCallFact>,
        loans: Arena<BorrowLoanFact>,
        states: Arena<StateBorrowFact>,
    ) -> Self {
        Self {
            writable_roots,
            access_segments,
            owner_segments,
            argument_accesses,
            calls,
            loans,
            states,
        }
    }

    pub fn access_segments(&self, access: &BorrowArgumentAccessFact) -> &[psi_facts::PlaceSegment] {
        self.access_segments.span_or_empty(access.segments)
    }

    pub fn accesses_overlap(
        &self,
        left: &BorrowArgumentAccessFact,
        right: &BorrowArgumentAccessFact,
    ) -> bool {
        left.root_symbol == right.root_symbol
            && place_segments_overlap(self.access_segments(left), self.access_segments(right))
    }

    pub fn loan_segments(&self, loan: &BorrowLoanFact) -> &[psi_facts::PlaceSegment] {
        self.access_segments.span_or_empty(loan.segments)
    }

    pub fn loan_owner_path(&self, loan: &BorrowLoanFact) -> &[BorrowLoanOwnerSegment] {
        self.owner_segments.span_or_empty(loan.owner_path)
    }

    pub fn access_overlaps_loan(
        &self,
        access: &BorrowArgumentAccessFact,
        loan: &BorrowLoanFact,
    ) -> bool {
        access.root_symbol == loan.root_symbol
            && place_segments_overlap(self.access_segments(access), self.loan_segments(loan))
    }

    pub fn loan_overlaps_loan(&self, left: &BorrowLoanFact, right: &BorrowLoanFact) -> bool {
        left.root_symbol == right.root_symbol
            && place_segments_overlap(self.loan_segments(left), self.loan_segments(right))
    }
}

fn place_segments_overlap(
    left: &[psi_facts::PlaceSegment],
    right: &[psi_facts::PlaceSegment],
) -> bool {
    let shared_len = left.len().min(right.len());
    left.iter()
        .take(shared_len)
        .zip(right.iter().take(shared_len))
        .all(|(left_segment, right_segment)| left_segment == right_segment)
}

#[cfg(test)]
mod tests {
    use crate::{
        BorrowArgumentAccessFact, BorrowCallFact, BorrowFacts, BorrowLoanFact,
        BorrowLoanOwnerSegment, BorrowWritableRootFact, StateBorrowFact,
    };
    use psi_arena::Arena;

    #[test]
    fn borrow_facts_constructor_keeps_borrow_roots_explicit() {
        let writable_roots = Arena::<BorrowWritableRootFact>::with_capacity(1);
        let access_segments = Arena::<psi_facts::PlaceSegment>::with_capacity(2);
        let owner_segments = Arena::<BorrowLoanOwnerSegment>::with_capacity(3);
        let argument_accesses = Arena::<BorrowArgumentAccessFact>::with_capacity(4);
        let calls = Arena::<BorrowCallFact>::with_capacity(5);
        let loans = Arena::<BorrowLoanFact>::with_capacity(6);
        let states = Arena::<StateBorrowFact>::with_capacity(7);

        let facts = BorrowFacts::with_roots(
            writable_roots.clone(),
            access_segments.clone(),
            owner_segments.clone(),
            argument_accesses.clone(),
            calls.clone(),
            loans.clone(),
            states.clone(),
        );

        assert_eq!(facts.writable_roots, writable_roots);
        assert_eq!(facts.access_segments, access_segments);
        assert_eq!(facts.owner_segments, owner_segments);
        assert_eq!(facts.argument_accesses, argument_accesses);
        assert_eq!(facts.calls, calls);
        assert_eq!(facts.loans, loans);
        assert_eq!(facts.states, states);
    }
}
