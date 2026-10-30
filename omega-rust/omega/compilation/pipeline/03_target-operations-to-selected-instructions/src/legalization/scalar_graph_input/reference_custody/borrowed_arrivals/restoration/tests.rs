use super::*;

fn leaf(number: u64, parents: Vec<OccurrenceIdentity>) -> Leaf {
    let result = StructuralOperationResult {
        place: PlaceId::new(number).unwrap(),
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
        qualification_establishments: vec![],
        claims: vec![],
    };
    Leaf::formed(
        (result.place, vec![]),
        Root::Place(PlaceId::new(99).unwrap()),
        parents,
        OperationId::new(number).unwrap(),
        &result,
        result.structural_type,
    )
}

fn event(number: u64) -> retirements::DisposalEvent {
    retirements::DisposalEvent::Edge(EdgeId::new(number).unwrap())
}

#[test]
fn only_selected_disposal_restores_original_parent() {
    let parent = leaf(1, vec![]).identity;
    let child = leaf(2, vec![parent.clone()]);
    let grant = child.identity.clone();
    let mut custody = Custody::default();
    custody.retire(child, event(1)).unwrap();
    assert_eq!(
        restored_parents(&custody, &[grant.clone()], event(2)).unwrap(),
        [grant.clone()]
    );
    assert_eq!(
        restored_parents(&custody, &[grant], event(1)).unwrap(),
        [parent]
    );
}

#[test]
fn live_descendants_and_independent_validity_keep_the_grant() {
    let closed = leaf(1, vec![]);
    let grant = closed.identity.clone();
    let mut custody = Custody::default();
    custody.retire(closed, event(1)).unwrap();
    let child = leaf(2, vec![grant.clone()]);
    custody.leaves.insert((child.identity.place, vec![]), child);
    assert_eq!(
        restored_parents(&custody, &[grant.clone()], event(1)).unwrap(),
        [grant.clone()]
    );
    custody.leaves.clear();
    custody
        .retired
        .get_mut(&grant)
        .unwrap()
        .leaf
        .lifetime_debts
        .push(lifetime_debts::Debt {
            backing: vec![],
            access: StructuralAccess::SharedBorrow,
            parents: vec![],
        });
    assert_eq!(
        restored_parents(&custody, &[grant.clone()], event(1)).unwrap(),
        [grant]
    );
}

#[test]
fn different_backing_and_cyclic_ancestry_do_not_disappear() {
    let closed = leaf(1, vec![]);
    let grant = closed.identity.clone();
    let mut custody = Custody::default();
    custody.retire(closed, event(1)).unwrap();
    custody.retired.get_mut(&grant).unwrap().leaf.backing.push(
        descriptor_backing::Backing::Source(Root::Place(PlaceId::new(98).unwrap())),
    );
    assert_eq!(
        restored_parents(&custody, &[grant.clone()], event(1)).unwrap(),
        [grant.clone()]
    );
    let retired = custody.retired.get_mut(&grant).unwrap();
    retired.leaf.backing.pop();
    retired.leaf.parents.push(grant.clone());
    assert!(matches!(
        restored_parents(&custody, &[grant], event(1)),
        Err(LegalizationError::ReferenceCompletion {
            reason: terminal_semantics::ReferenceCompletionError::CyclicAncestry
        })
    ));
}
