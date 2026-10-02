use super::*;

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}

fn empty_projection() -> Projection {
    Projection {
        path: Vec::new(),
        exact: false,
        indirect: false,
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
    let mut roots = regions.iter().filter_map(|region| region.root).collect::<Vec<_>>();
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
    };
    let aliases = AliasSources::from([
        (place(1), vec![(place(1), drifting.clone()), (place(2), empty_projection())]),
        (place(2), vec![(place(3), empty_projection())]),
    ]);
    let regions = resolve(&aliases, place(1), empty_projection());
    let roots = regions.iter().filter_map(|region| region.root).collect::<Vec<_>>();
    assert_eq!(roots, vec![place(3)]);
}
