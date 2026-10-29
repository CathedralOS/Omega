use super::ElementView;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::{PlaceId, StructuralDomainId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralPathQualification, StructuralPathSegment};

#[test]
fn descriptor_home_survives_checked_membership_changes() {
    let original =
        TerminalStructuralValue::minted(17, StructuralTypeId::new(3).unwrap(), Vec::new());
    let view = ElementView::structural(
        original.clone(),
        4,
        StructuralAccess::MutableBorrow,
        None,
        PlaceId::new(9).unwrap(),
    )
    .with_descriptor_home(original.clone());
    let mut qualified = original.clone();
    qualified
        .qualifications
        .push(StructuralDomainId::new(2).unwrap());
    qualified
        .projected_qualifications
        .push(StructuralPathQualification {
            path: vec![StructuralPathSegment::FixedIndex(1)],
            domain: StructuralDomainId::new(5).unwrap(),
        });
    assert!(view.matches_descriptor_home(&qualified));
    assert!(view.matches_bound_descriptor_home(&original, &qualified));
    assert!(ElementView::same_descriptor_home(&qualified, &original));
    assert!(original.qualifications.is_empty());
    assert!(original.projected_qualifications.is_empty());
}

#[test]
fn descriptor_home_keeps_occurrence_path_and_carrier_distinct() {
    let original =
        TerminalStructuralValue::minted(17, StructuralTypeId::new(3).unwrap(), Vec::new());
    let view = ElementView::structural(
        original.clone(),
        4,
        StructuralAccess::MutableBorrow,
        None,
        PlaceId::new(9).unwrap(),
    )
    .with_descriptor_home(original.clone());
    let mut another = original.clone();
    another.opaque_identity += 1;
    assert!(!view.matches_descriptor_home(&another));
    assert!(!view.matches_bound_descriptor_home(&original, &another));
    let mut projected = original.clone();
    projected.path.push(StructuralPathSegment::FixedIndex(0));
    assert!(!view.matches_descriptor_home(&projected));
    let mut recast = original.clone();
    recast.structural_type = StructuralTypeId::new(4).unwrap();
    assert!(!view.matches_descriptor_home(&recast));
}
