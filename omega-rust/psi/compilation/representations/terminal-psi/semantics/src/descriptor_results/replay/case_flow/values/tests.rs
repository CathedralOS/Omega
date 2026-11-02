use super::*;

fn location(root: u64) -> DescriptorLocation {
    DescriptorLocation::whole(PlaceId::new(root).unwrap())
}
fn members(roots: &[u64]) -> BTreeSet<DescriptorLocation> {
    roots.iter().copied().map(location).collect()
}

#[test]
fn retiring_representatives_preserves_old_cuts_and_rekeys_current_relations() {
    let mut copies = Copies::default();
    copies.insert_class(members(&[1, 2, 3]), 100).unwrap();
    copies.insert_class(members(&[70, 71]), 100).unwrap();
    let before = copies.clone();
    let untouched = copies.classes.get(&location(70)).unwrap().clone();
    copies.retain(|member| member != &location(1));
    assert_eq!(
        copies.equivalents(&location(2), 100),
        Some(members(&[2, 3]))
    );
    assert_eq!(copies.equivalents(&location(1), 100), Some(members(&[1])));
    assert_eq!(
        before.equivalents(&location(1), 100),
        Some(members(&[1, 2, 3]))
    );
    assert!(Arc::ptr_eq(
        copies.classes.get(&location(70)).unwrap(),
        &untouched
    ));
    copies.retain(|member| member != &location(2));
    assert_eq!(copies.counts(), (1, 2));
    assert_eq!(copies.equivalents(&location(3), 100), Some(members(&[3])));
}

#[test]
fn meeting_relations_keeps_only_common_pairs_and_preserves_each_arrival() {
    let mut left = Copies::default();
    left.insert_class(members(&[1, 2, 3, 4]), 100).unwrap();
    let mut right = Copies::default();
    right.insert_class(members(&[1, 3]), 100).unwrap();
    right.insert_class(members(&[2, 4, 5]), 100).unwrap();
    let result = left.meet(&right);
    assert_eq!(
        result.equivalents(&location(1), 100),
        Some(members(&[1, 3]))
    );
    assert_eq!(
        result.equivalents(&location(2), 100),
        Some(members(&[2, 4]))
    );
    assert_eq!(result.equivalents(&location(5), 100), Some(members(&[5])));
    assert_eq!(
        left.equivalents(&location(1), 100),
        Some(members(&[1, 2, 3, 4]))
    );
    assert_eq!(
        right.equivalents(&location(5), 100),
        Some(members(&[2, 4, 5]))
    );
    assert!(left.meet(&right) == right.meet(&left));
}

#[test]
fn extending_relations_preserves_snapshot_rosters_and_inline_suffixes() {
    let mut copies = Copies::default();
    copies.insert_class(members(&[1, 2]), 100).unwrap();
    let before = copies.clone();
    copies.insert_class(members(&[2, 3]), 100).unwrap();
    let mut subject = location(1);
    subject.path.push(DescriptorPathSegment::FixedIndex(7));
    let expected = members(&[1, 2, 3])
        .into_iter()
        .map(|mut member| {
            member.path.push(DescriptorPathSegment::FixedIndex(7));
            member
        })
        .collect();
    assert_eq!(copies.equivalents(&subject, 100), Some(expected));
    assert_eq!(
        before.equivalents(&location(2), 100),
        Some(members(&[1, 2]))
    );
    subject.path.push(DescriptorPathSegment::Referent);
    assert_eq!(
        copies.equivalents(&subject, 100),
        Some(BTreeSet::from([subject]))
    );
}
