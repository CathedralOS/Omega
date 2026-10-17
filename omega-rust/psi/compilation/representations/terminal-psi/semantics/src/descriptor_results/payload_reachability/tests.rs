use super::*;
use crate::descriptor_lifetimes::DescriptorFlowEdge;
use semantic_vocabulary::BlockId;

fn position(block: u64) -> DescriptorPosition {
    (BlockId::new(block).unwrap(), 0)
}
fn value(index: u64) -> ValueId {
    ValueId::new(index).unwrap()
}
fn boolean(index: u64) -> ScalarTerm {
    ScalarTerm::value(value(index), ScalarType::Boolean)
}
fn selected(index: u64, polarity: bool) -> Option<Proposition> {
    Some(Proposition::Equal(
        ScalarTerm::Boolean(polarity),
        boolean(index),
    ))
}
fn step(targets: &[u64]) -> DescriptorFlowStep {
    DescriptorFlowStep {
        successors: targets
            .iter()
            .map(|target| DescriptorFlowEdge {
                target: position(*target),
                parameters: Vec::new(),
                bindings: Vec::new(),
            })
            .collect(),
        ..Default::default()
    }
}

#[derive(Default)]
struct Graph {
    flow: BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    predicates: BTreeMap<DescriptorPosition, Vec<Option<Proposition>>>,
    bindings: BTreeMap<DescriptorPosition, Vec<Vec<(ValueId, ScalarTerm)>>>,
    parameters: BTreeMap<BlockId, Vec<ValueId>>,
    definitions: BTreeMap<DescriptorPosition, Vec<ValueId>>,
}
impl Graph {
    fn query(&self) -> QueryFlow<'_> {
        QueryFlow {
            flow: &self.flow,
            predicates: &self.predicates,
            bindings: &self.bindings,
            parameters: &self.parameters,
            definitions: &self.definitions,
        }
    }
}

#[test]
fn differently_bound_edges_to_one_target_remain_distinct() {
    let mut graph = Graph {
        flow: BTreeMap::from([
            (position(1), step(&[2, 2])),
            (position(2), step(&[3, 4])),
            (position(3), step(&[])),
            (position(4), step(&[])),
        ]),
        predicates: BTreeMap::from([(position(2), vec![selected(1, true), selected(1, false)])]),
        bindings: BTreeMap::from([(
            position(1),
            vec![
                vec![(value(1), ScalarTerm::Boolean(true))],
                vec![(value(1), ScalarTerm::Boolean(false))],
            ],
        )]),
        parameters: BTreeMap::from([(position(2).0, vec![value(1)])]),
        ..Default::default()
    };
    let reachable = graph.query().predecessors(position(3)).unwrap();
    assert!(reachable.includes_edge(position(1), 0));
    assert!(!reachable.includes_edge(position(1), 1));
    assert!(!reachable.contains(position(4)));
    // Unknown computations do not prove impossibility.
    graph.bindings.get_mut(&position(1)).unwrap()[1][0].1 = boolean(7);
    let reachable = graph.query().predecessors(position(3)).unwrap();
    assert!(reachable.includes_edge(position(1), 1));
    graph.bindings.get_mut(&position(1)).unwrap()[1][0].1 =
        ScalarTerm::boolean_equal(boolean(7), boolean(8)).unwrap();
    assert!(
        graph
            .query()
            .predecessors(position(3))
            .unwrap()
            .includes_edge(position(1), 1)
    );
    graph.bindings.clear();
    assert!(
        graph
            .query()
            .predecessors(position(3))
            .unwrap()
            .includes_edge(position(1), 1)
    );
}

#[test]
fn pre_definition_and_post_definition_queries_observe_different_instances() {
    let before = position(1);
    let after = (before.0, 1);
    let mut operation = step(&[]);
    operation.successors.push(DescriptorFlowEdge {
        target: after,
        parameters: Vec::new(),
        bindings: Vec::new(),
    });
    let graph = Graph {
        flow: BTreeMap::from([
            (before, operation),
            (after, step(&[2, 3])),
            (position(2), step(&[])),
            (position(3), step(&[])),
        ]),
        predicates: BTreeMap::from([(after, vec![selected(1, true), selected(1, false)])]),
        definitions: BTreeMap::from([(before, vec![value(1)])]),
        ..Default::default()
    };
    assert!(!graph.query().predecessors(before).unwrap().contains(after));
    assert!(graph.query().predecessors(after).unwrap().contains(before));
    let requirements = BTreeMap::from([(value(1), true)]);
    assert_eq!(
        graph.query().before_edge(before, 0, after, &requirements),
        Some(BTreeMap::new())
    );
}

#[test]
fn parameter_rebinding_is_simultaneous_and_missing_bindings_forget_demands() {
    let mut graph = Graph {
        parameters: BTreeMap::from([(position(2).0, vec![value(1), value(2)])]),
        bindings: BTreeMap::from([(
            position(1),
            vec![vec![(value(1), boolean(2)), (value(2), boolean(1))]],
        )]),
        ..Default::default()
    };
    let demands = BTreeMap::from([(value(1), true), (value(2), false)]);
    assert_eq!(
        graph
            .query()
            .before_edge(position(1), 0, position(2), &demands),
        Some(BTreeMap::from([(value(1), false), (value(2), true)]))
    );
    graph.bindings.clear();
    assert_eq!(
        graph
            .query()
            .before_edge(position(1), 0, position(2), &demands),
        Some(BTreeMap::new())
    );
}

#[test]
fn query_keeps_backedges_and_retires_new_scalar_instances() {
    let mut graph = Graph {
        flow: BTreeMap::from([
            (position(1), step(&[2])),
            (position(2), step(&[3, 5])),
            (position(3), step(&[4])),
            (position(4), step(&[2])),
            (position(5), step(&[])),
        ]),
        predicates: BTreeMap::from([(position(2), vec![selected(1, true), selected(1, false)])]),
        parameters: BTreeMap::from([(position(2).0, vec![value(1)])]),
        bindings: BTreeMap::from([
            (
                position(1),
                vec![vec![(value(1), ScalarTerm::Boolean(true))]],
            ),
            (position(4), vec![vec![(value(1), boolean(7))]]),
        ]),
        definitions: BTreeMap::from([(position(4), vec![value(7)])]),
    };
    let reachable = graph.query().predecessors(position(3)).unwrap();
    assert!(reachable.includes_edge(position(3), 0));
    assert!(reachable.includes_edge(position(4), 0));
    assert!(!reachable.contains(position(5)));
    let demands = BTreeMap::from([(value(7), true)]);
    assert_eq!(
        graph
            .query()
            .before_edge(position(4), 0, position(3), &demands),
        Some(BTreeMap::new())
    );
    graph.definitions.clear();
    assert_eq!(
        graph
            .query()
            .before_edge(position(4), 0, position(3), &demands),
        Some(demands)
    );
}

#[test]
fn conflicting_unknown_successors_weaken_demands_and_reopen_predecessors() {
    let graph = Graph {
        flow: BTreeMap::from([
            (position(1), step(&[2])),
            (position(2), step(&[3, 4])),
            (position(3), step(&[5])),
            (position(4), step(&[6])),
            (position(6), step(&[5])),
            (position(5), step(&[])),
        ]),
        predicates: BTreeMap::from([(position(2), vec![selected(1, true), selected(1, false)])]),
        parameters: BTreeMap::from([(position(2).0, vec![value(1)])]),
        bindings: BTreeMap::from([(
            position(1),
            vec![vec![(value(1), ScalarTerm::Boolean(false))]],
        )]),
        ..Default::default()
    };
    let reachable = graph.query().predecessors(position(5)).unwrap();
    assert!(reachable.includes_edge(position(1), 0));
    assert!(reachable.includes_edge(position(2), 0));
    assert!(reachable.includes_edge(position(2), 1));
}
