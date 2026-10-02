use super::*;
use semantic_vocabulary::StructuralTypeId;

fn collection() -> TerminalStructuralValue {
    TerminalStructuralValue {
        opaque_identity: 991,
        structural_type: StructuralTypeId::new(1).unwrap(),
        qualifications: Vec::new(),
        path: vec![StructuralPathSegment::Field("rooms".into())],
    }
}

#[test]
fn structural_element_view_keeps_storage_identity_across_descriptor_copies() {
    let backing = collection();
    let view = ElementView::structural(backing.clone(), 3, StructuralAccess::MutableBorrow);
    let transferred = view.clone();
    let path = [
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Field("nested".into()),
    ];
    let (retained, selected) = transferred.structural_projection(&path, true).unwrap();
    assert_eq!(retained, backing);
    assert_eq!(selected, path);
    assert_eq!(view.len(), 3);
    assert!(
        view.get(1).is_none(),
        "structural storage is not a scalar copy"
    );
}

#[test]
fn structural_element_view_window_preserves_offsets_and_attenuates_access() {
    let backing = collection();
    let view = ElementView::structural(backing.clone(), 4, StructuralAccess::MutableBorrow);
    let window = view.subslice(1, 4).unwrap().subslice(1, 3).unwrap();
    let (retained, path) = window
        .structural_projection(&[StructuralPathSegment::FixedIndex(1)], false)
        .unwrap();
    assert_eq!(retained, backing);
    assert_eq!(path, [StructuralPathSegment::FixedIndex(3)]);
    assert!(
        window
            .structural_projection(&[StructuralPathSegment::FixedIndex(0)], true)
            .is_err()
    );
    assert!(
        window
            .structural_projection(&[StructuralPathSegment::FixedIndex(2)], false)
            .is_err()
    );
    assert!(window.structural_projection(&[], false).is_err());
    assert!(window.subslice(0, 3).is_none());
    assert!(window.subslice(2, 1).is_none());
    let empty = window.subslice(2, 2).unwrap();
    assert_eq!(empty.len(), 0);
    assert!(
        empty
            .structural_projection(&[StructuralPathSegment::FixedIndex(0)], false)
            .is_err()
    );
}
