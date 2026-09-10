use super::*;

#[test]
fn anonymous_match_results_land_in_actual_integer_peer_without_erasing_dispatch() {
    let checked = checked_source(
        "machine choose(subject: bool, peer: u32) -> u32 {
            (match subject { true -> 1, _ -> 2 }) | peer
        }",
        false,
    );
    let plans = &checked.facts.values.scalar_computations;
    let root = plans.roots.iter().next().unwrap().1;
    let CheckedScalarComputationKind::Apply { operands, .. } = plans.nodes.get(root.root).kind
    else {
        panic!("ordinary enclosing integer operation");
    };
    let operands = plans.operands.span(operands).unwrap();
    let dispatch = plans.nodes.get(operands[0]);
    assert_eq!(dispatch.primitive_type, PrimitiveType::U32);
    assert!(matches!(
        dispatch.kind,
        CheckedScalarComputationKind::Dispatch { .. }
    ));
}

#[test]
fn scalar_match_retains_one_subject_and_selective_result_computations() {
    let checked = checked_source(
        "machine identity(value: bool) -> bool { value }
         machine choose(value: bool) -> bool {
             match identity(value) { true -> identity(false), false -> identity(true) }
         }",
        false,
    );
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .roots
        .iter()
        .find_map(|(_, root)| {
            matches!(
                plans.nodes.get(root.root).kind,
                CheckedScalarComputationKind::Dispatch { .. }
            )
            .then_some(root)
        })
        .expect("match root");
    let CheckedScalarComputationKind::Dispatch { subject, arms, .. } =
        plans.nodes.get(root.root).kind
    else {
        panic!("direct dispatch");
    };
    assert!(matches!(
        plans.nodes.get(subject).kind,
        CheckedScalarComputationKind::Call { .. }
    ));
    let arms = plans.dispatch_arms.span(arms).unwrap();
    assert_eq!(arms.len(), 2);
    for arm in arms {
        assert_ne!(subject, arm.value);
        assert!(matches!(
            plans.nodes.get(arm.value).kind,
            CheckedScalarComputationKind::Call { .. }
        ));
    }
    assert_ne!(arms[0].source_arm, arms[1].source_arm);
}

#[test]
fn scalar_match_wildcard_closes_executable_prefix_without_erasing_subject() {
    let checked = checked_source(
        "machine identity(value: bool) -> bool { value }
         machine choose(value: bool) -> bool {
             match identity(value) { _ -> false, true -> identity(true) }
         }",
        false,
    );
    let plans = &checked.facts.values.scalar_computations;
    let (subject, arms) = plans
        .nodes
        .iter()
        .find_map(|(_, node)| match node.kind {
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => Some((subject, arms)),
            _ => None,
        })
        .expect("dispatch remains even with wildcard first");
    assert!(matches!(
        plans.nodes.get(subject).kind,
        CheckedScalarComputationKind::Call { .. }
    ));
    let arms = plans.dispatch_arms.span(arms).unwrap();
    assert_eq!(arms.len(), 1);
    assert_eq!(
        arms[0].pattern,
        checked_trees::CheckedScalarDispatchPattern::Wildcard
    );
}
