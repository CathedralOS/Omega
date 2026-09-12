use super::*;
use std::cell::Cell;

#[test]
fn empty_static_provenance_does_not_request_a_call_frame() {
    let queries = Cell::new(0);
    let mut paths = Vec::new();
    let retired = retain_static_paths_across_call_frame(
        &typed_trees::TypedTrees::default(),
        &typed_trees::state::State::default(),
        &[],
        &mut paths,
        &[],
        || {
            queries.set(queries.get() + 1);
            Some(Vec::new())
        },
    );
    assert!(!retired);
    assert!(paths.is_empty());
    assert_eq!(
        queries.get(),
        0,
        "there are no facts for the frame to invalidate"
    );
}

#[test]
fn either_static_frontier_still_requires_invalidation() {
    let field = SymbolHandle::from_parts(1, 1);
    let local = crate::flow::canonical_place_from_symbol(field).expect("local marker");
    for (mut paths, local_places) in [
        (
            vec![StaticPersistentPath {
                field,
                segments: Vec::new(),
            }],
            Vec::new(),
        ),
        (Vec::new(), vec![local]),
    ] {
        let queried = Cell::new(false);
        let retired = retain_static_paths_across_call_frame(
            &typed_trees::TypedTrees::default(),
            &typed_trees::state::State::default(),
            &[],
            &mut paths,
            &local_places,
            || {
                queried.set(true);
                None
            },
        );
        assert!(queried.get());
        assert!(
            retired,
            "an opaque call retires local canonical markers too"
        );
        assert!(paths.is_empty());
    }
}
