use super::*;

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}

fn empty_projection() -> Projection {
    Projection {
        path: Vec::new(),
        exact: false,
        indirect: false,
        window: None,
        captured_at: None,
    }
}

fn link(from: u64, tos: &[u64]) -> (PlaceId, Vec<(PlaceId, Projection)>) {
    (
        place(from),
        tos.iter()
            .map(|to| (place(*to), empty_projection()))
            .collect(),
    )
}

// Subslice/view establishments fan out over one backing root: every cursor
// aliases the buffer, so leaves sit under diamond paths that the worklist
// would otherwise re-walk once per path. The regions a resolve emits must not
// multiply with the path count.
#[test]
fn resolve_emits_each_diamond_leaf_once() {
    let aliases = AliasSources::from([
        link(1, &[2, 3]),
        link(2, &[4, 5]),
        link(3, &[5, 6]),
        link(4, &[7, 8]),
        link(5, &[7, 8]),
        link(6, &[7, 8]),
    ]);
    let regions = resolve(&aliases, place(1), empty_projection());
    let mut roots = regions
        .iter()
        .filter_map(|region| region.root)
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    assert_eq!(roots, vec![place(7), place(8)]);
    assert_eq!(regions.len(), 2);
}

// A self-edge is cut by the path-local visited set even when its projection
// drifts; the expansion dedup must not let the cycle through or end the walk
// before sibling edges resolve.
#[test]
fn resolve_cuts_cycles_and_still_resolves_siblings() {
    let drifting = Projection {
        path: vec![StructuralPathSegment::Field("field".to_owned())],
        exact: true,
        indirect: false,
        window: None,
        captured_at: None,
    };
    let aliases = AliasSources::from([
        (
            place(1),
            vec![(place(1), drifting.clone()), (place(2), empty_projection())],
        ),
        (place(2), vec![(place(3), empty_projection())]),
    ]);
    let regions = resolve(&aliases, place(1), empty_projection());
    let roots = regions
        .iter()
        .filter_map(|region| region.root)
        .collect::<Vec<_>>();
    assert_eq!(roots, vec![place(3)]);
}

#[test]
fn captured_window_composes_only_through_exact_backing_prefixes() {
    let birth = (BlockId::new(1).unwrap(), 3);
    let start = ValueId::new(1).unwrap();
    let end = ValueId::new(2).unwrap();
    let window = Projection::window(start, end, birth);
    let prefix = Projection::new(&[StructuralPathSegment::Field("items".into())]);
    let captured = prefix.then(&window);
    assert_eq!(captured.path, prefix.path);
    assert_eq!(captured.window, Some((start, end)));
    assert_eq!(captured.captured_at, Some(birth));
    // The child's coordinates are relative to this descriptor, not its root.
    // Until an offset relation is established, retain the outer footprint.
    let child = Projection::window(
        ValueId::new(3).unwrap(),
        ValueId::new(4).unwrap(),
        (birth.0, 5),
    );
    let nested = captured.then(&child);
    assert_eq!(nested.window, captured.window);
    assert_eq!(nested.captured_at, captured.captured_at);
    let unknown = empty_projection().then(&window);
    assert!(unknown.window.is_none());
    assert!(!unknown.exact);
}

#[test]
fn runtime_selector_keeps_its_capture_and_stops_relative_suffixes() {
    let birth = (BlockId::new(1).unwrap(), 2);
    let path = vec![
        StructuralPathSegment::Field("items".into()),
        StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(1).unwrap(),
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        },
        StructuralPathSegment::Field("value".into()),
    ];
    let selected = Projection::new(&path).at(birth);
    assert_eq!(selected.path, path[..2]);
    assert!(!selected.exact);
    let composed = selected.then(&Projection::new(&[StructuralPathSegment::FixedIndex(0)]));
    assert_eq!(composed.path, path[..2]);
    assert_eq!(composed.captured_at, Some(birth));
    // Merely retaining a selector never proves separation from another index.
    let selected = Region {
        root: Some(place(1)),
        projection: selected,
    };
    let sibling = Region {
        root: Some(place(1)),
        projection: Projection::new(&[
            StructuralPathSegment::Field("items".into()),
            StructuralPathSegment::FixedIndex(0),
        ]),
    };
    assert!(selected.overlaps(&sibling));
}
