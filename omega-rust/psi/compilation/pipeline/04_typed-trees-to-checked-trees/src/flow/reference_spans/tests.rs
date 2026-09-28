use crate::flow::reference_spans::append_flow_reference;
use crate::flow::reference_spans::filter_constraint_refs;
use crate::flow::retained_constraint_refs;
use arena::{Handle, HandleSpan};
use checked_trees::FlowConstraintRef;

#[test]
fn reference_selection_preserves_generation_and_rejects_whole_stale_spans() {
    let mut references = arena::Arena::<FlowConstraintRef>::default();
    let source = references.insert_many([FlowConstraintRef::default(); 3]);
    let middle = Handle::from_arena_index(source.start().arena_index() + 1);
    assert!(references.free(middle));
    let replacement = references.insert(FlowConstraintRef::default());
    assert_ne!(replacement.generation(), middle.generation());
    let invalid_sources = [
        source,
        HandleSpan::from_parts(Handle::invalid(), 1),
        HandleSpan::from_parts(Handle::from_arena_index(u32::MAX), 2),
        HandleSpan::from_parts(source.start(), u32::MAX),
        HandleSpan::empty(),
    ];
    for invalid in invalid_sources {
        assert_eq!(
            retained_constraint_refs(&references, invalid),
            HandleSpan::empty()
        );
        let before = references.len();
        assert_eq!(
            filter_constraint_refs(&mut references, invalid, |_| panic!(
                "invalid source reached predicate"
            )),
            HandleSpan::empty()
        );
        assert_eq!(references.len(), before);
    }
    let selected = HandleSpan::from_parts(replacement, 1);
    assert_eq!(
        filter_constraint_refs(&mut references, selected, |_| true),
        selected
    );
    let mut extended = selected;
    append_flow_reference(&mut references, &mut extended, FlowConstraintRef::default());
    assert_eq!(extended.start().generation(), 1);
    assert_eq!(references.span_or_empty(extended).len(), 2);
    assert_eq!(references.span_or_empty(selected).len(), 1);
}
