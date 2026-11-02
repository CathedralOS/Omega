use super::{InvocationTarget, RealizedInvocationTarget, SymbolHandle, extend_realized};

#[test]
fn realized_invocation_union_is_order_independent_without_merging_identities() {
    use RealizedInvocationTarget::{Abstract, Requirement};
    let first = SymbolHandle::from_parts(3, 1);
    let replacement = SymbolHandle::from_parts(3, 2);
    let second = SymbolHandle::from_parts(5, 1);
    let groups = [
        vec![Requirement(first), Abstract(InvocationTarget::Parameter(1))],
        vec![
            Requirement(replacement),
            Abstract(InvocationTarget::Service(first)),
            Requirement(first),
        ],
        vec![
            Abstract(InvocationTarget::Parameter(0)),
            Requirement(second),
        ],
    ];
    let mut expected = Vec::new();
    for group in &groups {
        extend_realized(&mut expected, group);
    }
    assert_eq!(expected.len(), 6);
    for order in [[2, 1, 0], [1, 0, 2], [1, 2, 0], [0, 2, 1], [2, 0, 1]] {
        let mut observed = Vec::new();
        for index in order {
            extend_realized(&mut observed, &groups[index]);
        }
        assert_eq!(observed, expected);
        extend_realized(&mut observed, &expected);
        assert_eq!(observed, expected);
    }
    assert!(
        groups
            .iter()
            .flatten()
            .all(|target| expected.contains(target))
    );
}
