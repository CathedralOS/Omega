use super::super::super::StructuralTerm;
use super::*;
use facts::{PlaceRoot, PlaceSegment};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;

fn coordinate(segments: Vec<CapturedStorageSegment>) -> CapturedStorageCoordinate {
    CapturedStorageCoordinate {
        root: PlaceRoot::Unknown,
        segments,
    }
}

fn slot(index: usize) -> CapturedStorageSegment {
    CapturedStorageSegment::Place(PlaceSegment::FixedIndex { index })
}

fn selector(value: u64) -> CapturedStorageSegment {
    CapturedStorageSegment::Index(StructuralTerm::Integer(BigInt::from_u64(value)))
}

fn observed(coordinate: CapturedStorageCoordinate, occurrence: u64) -> CapturedStorageValue {
    CapturedStorageValue {
        coordinate,
        entry: Some(StructuralTerm::Integer(BigInt::from_u64(occurrence))),
        current: None,
        selected_case: None,
        completed_read: None,
        backing: None,
        current_anchors: Vec::new(),
        anchor_state: SymbolHandle::invalid(),
    }
}

/// Every stored slot is found at its own position, and an absent one is not.
#[test]
fn finds_each_slot_and_no_absent_one() {
    let mut stored = StoredValues::default();
    for index in 0..300 {
        stored.push(observed(coordinate(vec![slot(index)]), index as u64));
    }
    for index in 0..300 {
        assert_eq!(stored.position(&coordinate(vec![slot(index)])), Some(index));
    }
    assert_eq!(stored.position(&coordinate(vec![slot(300)])), None);
    assert_eq!(stored.position(&coordinate(vec![slot(0), slot(0)])), None);
}

/// Index selectors share a key, so lookup must still tell their terms apart.
#[test]
fn distinguishes_index_selectors_with_equal_keys() {
    let mut stored = StoredValues::default();
    stored.push(observed(coordinate(vec![selector(7)]), 0));
    stored.push(observed(coordinate(vec![selector(9)]), 1));
    assert_eq!(stored.position(&coordinate(vec![selector(9)])), Some(1));
    assert_eq!(stored.position(&coordinate(vec![selector(7)])), Some(0));
    assert_eq!(stored.position(&coordinate(vec![selector(8)])), None);
}

/// A repeated coordinate resolves to its first position, as a front-to-back
/// scan does.
#[test]
fn repeated_coordinate_resolves_to_first_position() {
    let values = vec![
        observed(coordinate(vec![slot(1)]), 0),
        observed(coordinate(vec![slot(2)]), 1),
        observed(coordinate(vec![slot(1)]), 2),
    ];
    let stored = StoredValues::from_values(values);
    assert_eq!(stored.position(&coordinate(vec![slot(1)])), Some(0));
}

/// Removing values moves later ones down; lookups follow them.
#[test]
fn removal_keeps_lookups_exact() {
    let mut stored = StoredValues::default();
    for index in 0..10 {
        stored.push(observed(coordinate(vec![slot(index)]), index as u64));
    }
    stored.retain_mut(|value| value.coordinate != coordinate(vec![slot(3)]));
    assert_eq!(stored.position(&coordinate(vec![slot(3)])), None);
    assert_eq!(stored.position(&coordinate(vec![slot(4)])), Some(3));
    stored.push(observed(coordinate(vec![slot(3)]), 10));
    assert_eq!(stored.position(&coordinate(vec![slot(3)])), Some(9));
    assert_eq!(stored.len(), 10);
}
