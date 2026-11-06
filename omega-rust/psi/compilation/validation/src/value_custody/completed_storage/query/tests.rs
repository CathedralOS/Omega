use super::*;
use arena::HandleSpan;
use typed_trees::TypedTrees;

#[test]
fn only_explicit_entry_reaches_incoming_storage() {
    let program = TypedTrees::default();
    let mut control = FlowControlFacts::default();
    let source = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(Handle::from_arena_index(1)),
        segments: Vec::new(),
    };
    let entry = control.storage_snapshots.append(FlowStorageSnapshot {
        operation: FlowStorageOperation::Entry,
        ..Default::default()
    });
    let empty_write = FlowStorageOperation::Writes {
        bindings: HandleSpan::empty(),
        contents: HandleSpan::empty(),
        bindings_complete: true,
        contents_complete: true,
        endpoint: None,
    };
    let after_entry = control.storage_snapshots.append(FlowStorageSnapshot {
        previous: entry,
        operation: empty_write,
    });
    for snapshot in [entry, after_entry] {
        assert!(
            matches!(sources_at_snapshot(&program, &control, snapshot, &source, StoredObservation::Binding),
            Some(StoredOperand::Incoming(place)) if place == source)
        );
    }
    assert!(
        sources_at_snapshot(
            &program,
            &control,
            Handle::invalid(),
            &source,
            StoredObservation::Binding
        )
        .is_none()
    );
    let unknown = control
        .storage_snapshots
        .append(FlowStorageSnapshot::default());
    let missing = control.storage_snapshots.append(FlowStorageSnapshot {
        previous: Handle::invalid(),
        operation: empty_write,
    });
    let write = control
        .operand_origin_writes
        .insert_many([checked_trees::FlowOperandReferent {
            root: facts::PlaceRoot::Symbol(Handle::from_arena_index(2)),
            ..Default::default()
        }]);
    let nonoverlapping_missing = control.storage_snapshots.append(FlowStorageSnapshot {
        previous: Handle::invalid(),
        operation: FlowStorageOperation::Writes {
            bindings: write,
            contents: write,
            bindings_complete: true,
            contents_complete: true,
            endpoint: None,
        },
    });
    let predecessors = control
        .storage_predecessors
        .insert_many([entry, Handle::invalid()]);
    let joined_missing = control.storage_snapshots.append(FlowStorageSnapshot {
        operation: FlowStorageOperation::Join { predecessors },
        ..Default::default()
    });
    let malformed_entry = control.storage_snapshots.append(FlowStorageSnapshot {
        previous: entry,
        operation: FlowStorageOperation::Entry,
    });
    let cyclic = control.storage_snapshots.append(FlowStorageSnapshot {
        operation: empty_write,
        ..Default::default()
    });
    control.storage_snapshots.get_mut(cyclic).previous = cyclic;
    for snapshot in [
        unknown,
        missing,
        nonoverlapping_missing,
        joined_missing,
        malformed_entry,
        cyclic,
    ] {
        assert!(matches!(
            sources_at_snapshot(
                &program,
                &control,
                snapshot,
                &source,
                StoredObservation::Binding
            ),
            Some(StoredOperand::Unavailable)
        ));
    }
}

#[test]
fn typed_store_uses_the_completed_local_definition() {
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{
        machine::Machine,
        name::Identifier,
        state::State,
        statement::{StatementNode, TableLocalData},
        types::TypeReferenceNode,
    };
    let mut symbols = SymbolTableBuilder::default();
    let machine = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
    let state = symbols
        .insert_children(
            machine,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        )
        .start();
    let local = symbols
        .insert_children(
            state,
            [(SymbolKind::Local, SymbolNameRef::Static("pointer"))],
        )
        .start();
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: unit,
            access: language_core::ReferenceAccess::Shared,
            lifetime: None,
        });
    let statements = program
        .statement_table
        .insert_statements([StatementNode::LocalData(TableLocalData {
            symbol: local,
            name: Identifier::generated("pointer"),
            type_reference: reference,
            ..Default::default()
        })]);
    let mut owner = Machine {
        symbol: machine,
        name: Identifier::generated("owner"),
        ..Default::default()
    };
    program.push_machine_state(
        &mut owner,
        State {
            symbol: state,
            name: Identifier::generated("entry"),
            statement_nodes: statements,
            ..Default::default()
        },
    );
    program.push_machine(owner);
    let source = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(local),
        segments: Vec::new(),
    };
    assert!(crate::canonical_place_type_reference(&program, state, 0, &source).is_none());
    let mut control = FlowControlFacts::default();
    let entry = control.storage_snapshots.append(FlowStorageSnapshot {
        operation: FlowStorageOperation::Entry,
        ..Default::default()
    });
    let value = control.operands.append(checked_trees::FlowOperandFact {
        type_reference: reference,
        ..Default::default()
    });
    let store = control.storage_snapshots.append(FlowStorageSnapshot {
        previous: entry,
        operation: FlowStorageOperation::Store {
            state,
            statement_index: 0,
            receiving_type: reference,
            target: Handle::invalid(),
            destination: checked_trees::FlowOperandReferent {
                root: source.root,
                ..Default::default()
            },
            value,
            projection: HandleSpan::empty(),
        },
    });
    assert!(matches!(
        sources_at_snapshot(&program, &control, store, &source, StoredObservation::Binding),
        Some(StoredOperand::Value { capture, .. }) if capture == value
    ));
    assert!(matches!(
        sources_at_typed_snapshot(
            &program,
            &control,
            store,
            &source,
            StoredObservation::Binding,
            Default::default()
        ),
        Some(StoredOperand::Unavailable)
    ));
    let mut missing_owner = *control.storage_snapshots.get(store);
    if let FlowStorageOperation::Store { state, .. } = &mut missing_owner.operation {
        *state = Handle::invalid();
    }
    let missing_owner = control.storage_snapshots.append(missing_owner);
    assert!(matches!(
        sources_at_snapshot(
            &program,
            &control,
            missing_owner,
            &source,
            StoredObservation::Binding
        ),
        Some(StoredOperand::Unavailable)
    ));
}

#[test]
fn typed_store_endpoints_keep_descriptors_separate_from_their_payloads() {
    use language_core::ReferenceAccess;
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let shared = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: unit,
            access: ReferenceAccess::Shared,
            lifetime: None,
        });
    let outer = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: shared,
            access: ReferenceAccess::Mutable,
            lifetime: None,
        });
    let other = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: unit,
            access: ReferenceAccess::Mutable,
            lifetime: None,
        });
    let array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: shared,
            length: FixedArrayLength::Literal(2),
        });
    let mut control = FlowControlFacts::default();
    let source = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(Handle::from_arena_index(1)),
        segments: Vec::new(),
    };
    let entry = control.storage_snapshots.append(FlowStorageSnapshot {
        operation: FlowStorageOperation::Entry,
        ..Default::default()
    });
    let destination = checked_trees::FlowOperandReferent {
        root: source.root,
        ..Default::default()
    };
    let mut store = |receiving_type| {
        let value = control.operands.append(checked_trees::FlowOperandFact {
            type_reference: receiving_type,
            ..Default::default()
        });
        let writes = control.operand_origin_writes.insert_many([destination]);
        let written = control.storage_snapshots.append(FlowStorageSnapshot {
            previous: entry,
            operation: FlowStorageOperation::Writes {
                bindings: writes,
                contents: writes,
                bindings_complete: true,
                contents_complete: true,
                endpoint: Some(checked_trees::FlowStorageWriteEndpoint {
                    state: Default::default(),
                    statement_index: 0,
                    receiving_type,
                }),
            },
        });
        let snapshot = control.storage_snapshots.append(FlowStorageSnapshot {
            previous: written,
            operation: FlowStorageOperation::Store {
                state: Default::default(),
                statement_index: 0,
                receiving_type,
                target: Handle::invalid(),
                destination,
                value,
                projection: HandleSpan::empty(),
            },
        });
        (snapshot, value)
    };
    let (payload_store, _) = store(unit);
    let (inner_store, inner_value) = store(shared);
    let (aggregate_store, aggregate_value) = store(array);
    let (unrelated_store, _) = store(other);
    let (unknown_store, _) = store(Default::default());
    for expected in [shared, outer] {
        assert!(matches!(
            sources_at_typed_snapshot(
                &program,
                &control,
                payload_store,
                &source,
                StoredObservation::Binding,
                expected
            ),
            Some(StoredOperand::Incoming(_))
        ));
    }
    assert!(matches!(
        at_typed_snapshot(
            &program,
            &control,
            payload_store,
            &source,
            StoredObservation::Contents,
            unit
        ),
        Some(StoredOperand::Value { .. })
    ));
    assert!(matches!(
        sources_at_typed_snapshot(
            &program,
            &control,
            inner_store,
            &source,
            StoredObservation::Binding,
            outer
        ),
        Some(StoredOperand::Incoming(_))
    ));
    assert!(
        matches!(at_typed_snapshot(&program, &control, inner_store, &source,
        StoredObservation::Binding, shared), Some(StoredOperand::Value { capture, .. }) if capture == inner_value)
    );
    let element = CanonicalPlace {
        root: source.root,
        segments: vec![facts::PlaceSegment::FixedIndex { index: 0 }],
    };
    assert!(
        matches!(at_typed_snapshot(&program, &control, aggregate_store, &element,
        StoredObservation::Binding, shared), Some(StoredOperand::Value { capture, projection, .. })
            if capture == aggregate_value && projection == element.segments)
    );
    let element_path = control
        .operand_referent_segments
        .insert_many(element.segments.iter().copied());
    let element_write =
        control
            .operand_origin_writes
            .insert_many([checked_trees::FlowOperandReferent {
                root: source.root,
                segments: element_path,
                ..Default::default()
            }]);
    for (receiving_type, preserves) in [(unit, true), (shared, false)] {
        let written = control.storage_snapshots.append(FlowStorageSnapshot {
            previous: entry,
            operation: FlowStorageOperation::Writes {
                bindings: element_write,
                contents: element_write,
                bindings_complete: true,
                contents_complete: true,
                endpoint: Some(checked_trees::FlowStorageWriteEndpoint {
                    state: Default::default(),
                    statement_index: 0,
                    receiving_type,
                }),
            },
        });
        let selected = sources_at_typed_snapshot(
            &program,
            &control,
            written,
            &source,
            StoredObservation::InlineContents,
            array,
        );
        assert_eq!(
            matches!(selected, Some(StoredOperand::Incoming(_))),
            preserves
        );
        assert!(matches!(
            sources_at_typed_snapshot(
                &program,
                &control,
                written,
                &source,
                StoredObservation::Contents,
                array
            ),
            Some(StoredOperand::Unavailable)
        ));
    }
    let untyped = control.storage_snapshots.append(FlowStorageSnapshot {
        previous: entry,
        operation: FlowStorageOperation::Writes {
            bindings: element_write,
            contents: element_write,
            bindings_complete: true,
            contents_complete: true,
            endpoint: None,
        },
    });
    assert!(matches!(
        sources_at_typed_snapshot(
            &program,
            &control,
            untyped,
            &element,
            StoredObservation::Binding,
            shared
        ),
        Some(StoredOperand::Unavailable)
    ));
    for snapshot in [unknown_store, unrelated_store] {
        assert!(matches!(
            sources_at_typed_snapshot(
                &program,
                &control,
                snapshot,
                &source,
                StoredObservation::Binding,
                shared
            ),
            Some(StoredOperand::Unavailable)
        ));
    }
    assert!(matches!(
        sources_at_typed_snapshot(
            &program,
            &control,
            entry,
            &source,
            StoredObservation::Binding,
            Default::default()
        ),
        Some(StoredOperand::Unavailable)
    ));
}
