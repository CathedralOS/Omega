mod indexes;
mod segments;

use self::segments::place_segments_may_overlap;

pub(super) fn canonical_place_overlaps_loan(
    program: &omega_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    loan: &omega_checked_trees::BorrowLoanFact,
    borrow: &omega_checked_trees::BorrowFacts,
) -> bool {
    match place.root {
        omega_facts::PlaceRoot::Symbol(symbol) => {
            if symbol == loan.root_symbol {
                return place_segments_may_overlap(
                    program,
                    &place.segments,
                    borrow.loan_segments(loan),
                );
            }

            match place.segments.split_first() {
                Some((
                    omega_facts::PlaceSegment::Field {
                        symbol: field_symbol,
                    },
                    remaining,
                )) if *field_symbol == loan.root_symbol => {
                    place_segments_may_overlap(program, remaining, borrow.loan_segments(loan))
                }
                _ => false,
            }
        }
        omega_facts::PlaceRoot::Unknown
        | omega_facts::PlaceRoot::Expression(_)
        | omega_facts::PlaceRoot::TypeReference(_) => false,
    }
}

pub(super) fn borrow_accesses_overlap(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
    left: &omega_checked_trees::BorrowArgumentAccessFact,
    right: &omega_checked_trees::BorrowArgumentAccessFact,
) -> bool {
    left.root_symbol == right.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.access_segments(left),
            facts.borrow.access_segments(right),
        )
}

pub(super) fn borrow_access_overlaps_loan(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
    access: &omega_checked_trees::BorrowArgumentAccessFact,
    loan: &omega_checked_trees::BorrowLoanFact,
) -> bool {
    access.root_symbol == loan.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.access_segments(access),
            facts.borrow.loan_segments(loan),
        )
}

pub(super) fn borrow_loan_overlaps_loan(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
    left: &omega_checked_trees::BorrowLoanFact,
    right: &omega_checked_trees::BorrowLoanFact,
) -> bool {
    left.root_symbol == right.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.loan_segments(left),
            facts.borrow.loan_segments(right),
        )
}
