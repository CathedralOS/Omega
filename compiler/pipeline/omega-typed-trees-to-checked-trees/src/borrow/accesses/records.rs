use super::*;

pub(super) fn append_argument_access(
    access_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
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
