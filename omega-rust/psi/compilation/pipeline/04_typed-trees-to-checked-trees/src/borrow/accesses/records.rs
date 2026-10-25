use crate::borrow::accesses::BorrowAccessPlace;
use checked_trees::{BorrowAccessKind, BorrowArgumentAccessFact};

pub(super) fn append_argument_access(
    access_segments: &mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &mut arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut arena::HandleSpan<BorrowArgumentAccessFact>,
    access_place: BorrowAccessPlace,
    kind: BorrowAccessKind,
    end_before_access: Option<usize>,
    expression: checked_trees::expression::ExpressionHandle,
) {
    argument_accesses.append_to_span(
        accesses,
        BorrowArgumentAccessFact {
            expression,
            evaluation_start: None,
            root: access_place.root,
            segments: access_segments.insert_many(access_place.segments),
            kind,
            end_before_access,
            released_at: if end_before_access.is_some() {
                expression
            } else {
                Default::default()
            },
        },
    );
}
