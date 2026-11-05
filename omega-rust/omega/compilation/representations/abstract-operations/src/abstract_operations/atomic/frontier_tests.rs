use super::*;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

fn operation(edge: AtomicResidencyEdge) -> AbstractOperation {
    AbstractOperation::AtomicEvent {
        psi_operation: OperationId::new(1).unwrap(),
        event: AbstractAtomicEvent::ReadModifyWrite {
            location: AbstractAtomicLocation {
                root: PlaceId::new(1).unwrap(),
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
            },
            operation: AbstractAtomicReadModifyWrite::FetchAnd,
            ordering: MemoryOrdering::Receive,
            operand: ValueId::new(1).unwrap(),
            prior: AbstractResult {
                value: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                ),
            },
        },
        resident_obligation: None,
        reads_from: Some(edge.clone()),
        modification_after: Some(edge),
    }
}

fn alternatives() -> AtomicResidencyEdge {
    AtomicResidencyEdge::PathAlternatives {
        sources: vec![
            AtomicResidencySource::Initial,
            AtomicResidencySource::Write {
                operation: OperationId::new(1).unwrap(),
            },
        ],
    }
}

fn replay(
    edge: AtomicResidencyEdge,
    backedge: impl Fn(PlaceId) -> (PlaceId, Vec<PathSegment>),
) -> Option<(BlockId, usize, AtomicCoherenceViolation)> {
    let entry = BlockId::new(1).unwrap();
    let repeated = BlockId::new(2).unwrap();
    let operation = operation(edge);
    let blocks = BTreeMap::from([(entry, Vec::new()), (repeated, vec![&operation])]);
    let predecessors = BTreeMap::from([(repeated, BTreeSet::from([entry, repeated]))]);
    let dominators = BTreeMap::from([
        (entry, BTreeSet::from([entry])),
        (repeated, BTreeSet::from([entry, repeated])),
    ]);
    happens_before_atomic_coherence_violation(
        &blocks,
        &predecessors,
        &dominators,
        entry,
        |source, _, root| {
            Some(if source == repeated {
                backedge(root)
            } else {
                (root, Vec::new())
            })
        },
        |_, _, _, _| false,
    )
}

#[test]
fn loop_retains_initial_and_previous_iteration_residency() {
    assert_eq!(replay(alternatives(), |root| (root, Vec::new())), None);
}

#[test]
fn loop_cannot_claim_only_its_previous_iteration_write() {
    let edge = AtomicResidencyEdge::Write {
        operation: OperationId::new(1).unwrap(),
    };
    assert!(replay(edge, |root| (root, Vec::new())).is_some());
}

#[test]
fn alternatives_cannot_add_a_non_reaching_writer() {
    let edge = AtomicResidencyEdge::PathAlternatives {
        sources: vec![
            AtomicResidencySource::Initial,
            AtomicResidencySource::Write {
                operation: OperationId::new(99).unwrap(),
            },
        ],
    };
    assert!(replay(edge, |root| (root, Vec::new())).is_some());
}

#[test]
fn alternatives_reject_changed_backedge_storage() {
    assert!(replay(alternatives(), |_| (PlaceId::new(2).unwrap(), Vec::new())).is_some());
    assert!(replay(alternatives(), |root| (root, vec![PathSegment::Referent])).is_some());
}

#[test]
fn alternatives_must_be_canonical_and_nontrivial() {
    for sources in [
        vec![AtomicResidencySource::Initial],
        vec![
            AtomicResidencySource::Initial,
            AtomicResidencySource::Initial,
        ],
        vec![
            AtomicResidencySource::Write {
                operation: OperationId::new(1).unwrap(),
            },
            AtomicResidencySource::Initial,
        ],
    ] {
        assert!(
            replay(AtomicResidencyEdge::PathAlternatives { sources }, |root| (
                root,
                Vec::new()
            ))
            .is_some()
        );
    }
}

#[test]
fn alternatives_reject_a_defined_writer_that_cannot_reach_the_loop() {
    let entry = BlockId::new(1).unwrap();
    let repeated = BlockId::new(2).unwrap();
    let exit = BlockId::new(3).unwrap();
    let writer = OperationId::new(99).unwrap();
    let after_loop = AbstractOperation::AtomicEvent {
        psi_operation: writer,
        event: AbstractAtomicEvent::Store {
            location: AbstractAtomicLocation {
                root: PlaceId::new(1).unwrap(),
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
            },
            ordering: MemoryOrdering::Publish,
            value: ValueId::new(1).unwrap(),
        },
        resident_obligation: None,
        reads_from: None,
        modification_after: Some(AtomicResidencyEdge::Write {
            operation: OperationId::new(1).unwrap(),
        }),
    };
    let predecessors = BTreeMap::from([
        (repeated, BTreeSet::from([entry, repeated])),
        (exit, BTreeSet::from([repeated])),
    ]);
    let dominators = BTreeMap::from([
        (entry, BTreeSet::from([entry])),
        (repeated, BTreeSet::from([entry, repeated])),
        (exit, BTreeSet::from([entry, repeated, exit])),
    ]);
    let check = |edge| {
        let repeated_operation = operation(edge);
        let blocks = BTreeMap::from([
            (entry, Vec::new()),
            (repeated, vec![&repeated_operation]),
            (exit, vec![&after_loop]),
        ]);
        happens_before_atomic_coherence_violation(
            &blocks,
            &predecessors,
            &dominators,
            entry,
            |_, _, root| Some((root, Vec::new())),
            |_, _, _, _| false,
        )
    };
    assert_eq!(check(alternatives()), None);
    let claimed = AtomicResidencyEdge::PathAlternatives {
        sources: vec![
            AtomicResidencySource::Initial,
            AtomicResidencySource::Write { operation: writer },
        ],
    };
    assert_eq!(
        check(claimed),
        Some((
            repeated,
            0,
            AtomicCoherenceViolation::ReadsFrom(AtomicEdgeViolation::InvalidPathAlternatives)
        ))
    );
}
