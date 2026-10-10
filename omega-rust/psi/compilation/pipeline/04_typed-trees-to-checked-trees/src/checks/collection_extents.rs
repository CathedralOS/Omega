//! Read collection geometry from the live loan's frozen referent, never its
//! initializer. Element writes do not resize a fixed array or a captured
//! window. The caller supplies the occurrence's live loan constraints; an
//! expired or replaced descriptor therefore cannot resurrect old geometry.

use checked_trees::{BorrowFacts, FlowConstraintKind, FlowConstraintRef};
use validation::CanonicalPlace;

pub(crate) fn from_live_loans(
    program: &dyn ::validation::CheckingInput,
    state: symbols::SymbolHandle,
    statement_index: usize,
    borrow: &BorrowFacts,
    place: &CanonicalPlace,
    constraints: &[FlowConstraintRef],
) -> Option<usize> {
    let owner = place.root.as_symbol()?;
    let mut extent = None;
    for constraint in constraints {
        let FlowConstraintKind::BorrowLoan { loan } = constraint.kind else {
            continue;
        };
        let loan = borrow.loans.get(loan);
        let owner_path = borrow.loan_owner_path(loan);
        if loan.owner_symbol != owner || owner_path.len() > place.segments.len() {
            continue;
        }
        // Unknown owner selectors cannot select one descriptor's geometry.
        if !owner_path
            .iter()
            .zip(&place.segments)
            .all(|(owner, segment)| {
                matches!((owner, segment),
                (checked_trees::BorrowLoanOwnerSegment::Field(left),
                    facts::PlaceSegment::Field { symbol: right }) if left == right)
                    || matches!((owner, segment),
                (checked_trees::BorrowLoanOwnerSegment::Case(left),
                    facts::PlaceSegment::Case { variant: right }) if left == right)
                    || matches!((owner, segment),
                (checked_trees::BorrowLoanOwnerSegment::FixedIndex(left),
                    facts::PlaceSegment::FixedIndex { index: right }) if left == right)
            })
        {
            if crate::borrow::owner_path_overlaps_place_segments(
                program,
                owner_path,
                &place.segments,
            ) {
                // A possible descriptor cannot disappear merely because
                // another origin has a statically selectable extent.
                return None;
            }
            continue;
        }
        let mut referent = CanonicalPlace {
            root: facts::PlaceRoot::Symbol(loan.root_symbol),
            segments: borrow.loan_segments(loan).to_vec(),
        };
        referent
            .segments
            .extend_from_slice(&place.segments[owner_path.len()..]);
        crate::flow::compose_captured_view_segments(&mut referent.segments)?;
        let current = if let Some(facts::PlaceSegment::FixedRange { start, end }) =
            referent.segments.last()
        {
            end.checked_sub(*start)?
        } else {
            let reference = validation::canonical_place_type_reference(
                program,
                state,
                statement_index,
                &referent,
            )?;
            super::ranges::fixed_array_type_length(program, reference)?
        };
        if extent.is_some_and(|prior| prior != current) {
            return None;
        }
        extent = Some(current);
    }
    extent
}
