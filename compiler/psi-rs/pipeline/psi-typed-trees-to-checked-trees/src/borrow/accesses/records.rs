use super::*;

pub(super) fn append_argument_access(
    access_segments: &mut psi_arena::Arena<psi_facts::PlaceSegment>,
    argument_accesses: &mut psi_arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut psi_arena::HandleSpan<BorrowArgumentAccessFact>,
    access_place: BorrowAccessPlace,
    kind: BorrowAccessKind,
) {
    argument_accesses.append_to_span(
        accesses,
        BorrowArgumentAccessFact {
            root_symbol: access_place.root_symbol,
            segments: access_segments.insert_many(access_place.segments),
            kind,
        },
    );
}
