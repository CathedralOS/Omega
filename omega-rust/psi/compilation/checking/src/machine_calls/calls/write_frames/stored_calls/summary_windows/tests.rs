use super::coarsen_path;
use facts::PlaceSegment::{self, FixedIndex, FixedRange, OpaqueRange};

#[test]
fn fixed_windows_compose_in_the_original_dimension() {
    let mut path = vec![
        FixedRange { start: 2, end: 10 },
        FixedRange { start: 1, end: 5 },
    ];
    assert!(coarsen_path(&mut path, |_| false));
    assert_eq!(path, [FixedRange { start: 3, end: 7 }]);
    assert!(!coarsen_path(&mut path, |_| false));
}

#[test]
fn unknown_windows_converge_without_crossing_element_boundaries() {
    for unknown_position in 0..3 {
        let mut path = vec![FixedRange { start: 0, end: 7 }; 3];
        path[unknown_position] = OpaqueRange;
        path.extend([FixedIndex { index: 2 }, OpaqueRange, OpaqueRange]);
        assert!(coarsen_path(&mut path, |_| false));
        assert_eq!(path, [OpaqueRange, FixedIndex { index: 2 }, OpaqueRange]);
        assert!(!coarsen_path(&mut path, |_| false));
    }
}

#[test]
fn opaque_windows_do_not_swallow_a_different_descriptor_or_index() {
    let field = PlaceSegment::default();
    let index = PlaceSegment::Index {
        expression: Default::default(),
    };
    let mut path = vec![OpaqueRange, field, OpaqueRange, index, OpaqueRange];
    let before = path.clone();
    assert!(!coarsen_path(&mut path, |_| false));
    assert_eq!(path, before);
}
