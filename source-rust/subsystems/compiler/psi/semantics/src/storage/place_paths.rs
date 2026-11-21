pub use terminal_psi::types::descriptor_locations::may_overlap;

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId, ValueId};
    use terminal_psi::ResolvedStructuralPathSegment as Segment;

    #[test]
    fn views_overlap_original_fields_but_preserve_disjoint_outer_siblings() {
        let field = |id| Segment::Field(StructuralFieldId::new(id).unwrap());
        let view = Segment::RepresentationView {
            structural_type: StructuralTypeId::new(1).unwrap(),
            stored_scalar: None,
        };
        assert!(may_overlap(&[view, Segment::FixedIndex(0)], &[field(1)]));
        assert!(may_overlap(&[field(1)], &[view, Segment::FixedIndex(0)]));
        assert!(!may_overlap(&[field(1), view], &[field(2), view]));
        assert!(may_overlap(&[], &[view, Segment::FixedIndex(0)]));
        assert!(!may_overlap(
            &[view, Segment::FixedIndex(0)],
            &[view, Segment::FixedIndex(1)]
        ));
        assert!(may_overlap(
            &[view, Segment::FixedIndex(0)],
            &[
                view,
                Segment::RuntimeIndex {
                    selector: ValueId::new(1).unwrap()
                },
            ]
        ));
    }
}
