use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowArgumentAccessFact {
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
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
    pub source_owner_symbol: SymbolHandle,
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
    pub kind: BorrowAccessKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowFacts {
    pub writable_roots: Arena<BorrowWritableRootFact>,
    pub access_segments: Arena<omega_facts::PlaceSegment>,
    pub argument_accesses: Arena<BorrowArgumentAccessFact>,
    pub calls: Arena<BorrowCallFact>,
    pub loans: Arena<BorrowLoanFact>,
    pub states: Arena<StateBorrowFact>,
}

impl BorrowFacts {
    pub fn access_segments(
        &self,
        access: &BorrowArgumentAccessFact,
    ) -> &[omega_facts::PlaceSegment] {
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

    pub fn loan_segments(&self, loan: &BorrowLoanFact) -> &[omega_facts::PlaceSegment] {
        self.access_segments.span_or_empty(loan.segments)
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
    left: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
) -> bool {
    let shared_len = left.len().min(right.len());
    left.iter()
        .take(shared_len)
        .zip(right.iter().take(shared_len))
        .all(|(left_segment, right_segment)| left_segment == right_segment)
}
