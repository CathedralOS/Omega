//! Read collection geometry from the live loan's frozen referent, never its
//! initializer. Element writes do not resize a fixed array or a captured
//! window. Loan activation joins its completed value capture, so direct
//! indexing and call admission consume the same formation-time geometry.
//! The caller supplies the occurrence's live loan constraints; an
//! expired or replaced descriptor therefore cannot resurrect old geometry.

use checked_trees::{BorrowFacts, FlowConstraintKind, FlowConstraintRef};
use validation::CanonicalPlace;

/// The completed operand supplies descriptor geometry; the call's ordinary
/// loan and bounds checks still own access. Every possible referent must agree.
pub(crate) fn from_captured_value(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    flow: &checked_trees::FlowFacts,
    value: &crate::flow::CapturedValue<'_>,
    invocation: &[FlowConstraintRef],
) -> Option<usize> {
    let mut extent = None;
    for captured in crate::flow::captured_value_places(program, flow, value)? {
        let current = from_live_loans(
            program,
            value.operand.state_symbol,
            value.operand.statement_index,
            borrow,
            flow,
            &captured.projected,
            invocation,
        )
        .or_else(|| {
            at_formed_place(
                program,
                value.operand.state_symbol,
                value.operand.statement_index,
                &captured.projected,
            )
        })?;
        if extent.is_some_and(|prior| prior != current) {
            return None;
        }
        extent = Some(current);
    }
    extent
}

/// Static address geometry only. Bounded byte owners' capacity is not their
/// live length; the ordinary array owner explicitly excludes those carriers.
pub(crate) fn at_formed_place(
    program: &dyn ::validation::CheckingInput,
    state: symbols::SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
) -> Option<usize> {
    if let Some(facts::PlaceSegment::FixedRange { start, end }) = place.segments.last() {
        return end.checked_sub(*start);
    }
    let reference =
        validation::canonical_place_type_reference(program, state, statement_index, place)?;
    super::ranges::fixed_array_type_length(program, reference)
}

pub(crate) fn from_live_loans(
    program: &dyn ::validation::CheckingInput,
    state: symbols::SymbolHandle,
    statement_index: usize,
    borrow: &BorrowFacts,
    flow: &checked_trees::FlowFacts,
    place: &CanonicalPlace,
    constraints: &[FlowConstraintRef],
) -> Option<usize> {
    let owner = place.root.as_symbol()?;
    let mut extent = None;
    for constraint in constraints {
        let FlowConstraintKind::BorrowLoan { loan: loan_handle } = constraint.kind else {
            continue;
        };
        let loan = borrow.loans.get(loan_handle);
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
        let captured = flow
            .borrow_lifetimes
            .activations
            .iter()
            .find(|(_, activation)| activation.loan == loan_handle)
            .and_then(|(_, activation)| {
                flow.control
                    .operands
                    .is_valid(activation.formation_value)
                    .then(|| flow.control.operands.get(activation.formation_value))
            })
            .filter(|value| crate::flow::operand_binds_live_view(program, value.type_reference));
        let sources = if let Some(captured) = captured {
            let rows = flow
                .control
                .operand_referents
                .span_or_empty(captured.referents);
            if rows.is_empty() {
                return None;
            }
            rows.iter()
                .map(|referent| CanonicalPlace {
                    root: referent.root,
                    segments: flow
                        .control
                        .operand_referent_segments
                        .span_or_empty(referent.segments)
                        .to_vec(),
                })
                .collect::<Vec<_>>()
        } else {
            vec![CanonicalPlace {
                root: loan.root,
                segments: borrow.loan_segments(loan).to_vec(),
            }]
        };
        let mut loan_extent = None;
        for source in sources {
            let referent = crate::flow::project_captured_view_place(
                source,
                &place.segments[owner_path.len()..],
            )?;
            let current = at_formed_place(program, state, statement_index, &referent)?;
            if loan_extent.is_some_and(|prior| prior != current) {
                return None;
            }
            loan_extent = Some(current);
        }
        let current = loan_extent?;
        if extent.is_some_and(|prior| prior != current) {
            return None;
        }
        extent = Some(current);
    }
    extent
}
