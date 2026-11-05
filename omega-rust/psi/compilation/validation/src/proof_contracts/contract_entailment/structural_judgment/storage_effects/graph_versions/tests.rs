//! A read of unchanged input storage remains valid until a later write.
//! Unknown writes and joins containing them must retire the saved value.

use super::*;
use checked_trees::{FlowFacts, FlowStorageOperation};
use std::sync::Arc;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{TypedTrees, expression::ExpressionNode};

#[test]
fn completed_observation_of_incoming_storage_survives_only_preserved_cuts() {
    let mut symbols = SymbolTableBuilder::default();
    let state = symbols.insert_root(SymbolKind::State, SymbolNameRef::Static("entry"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let expression = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let coordinate = CapturedStorageCoordinate {
        root: facts::PlaceRoot::Expression(expression),
        segments: Vec::new(),
    };
    let mut flow = FlowFacts::default();
    let unchanged = FlowStorageOperation::Writes {
        bindings: Default::default(),
        contents: Default::default(),
        bindings_complete: true,
        contents_complete: true,
    };
    let observed = flow.control.storage_snapshots.insert(FlowStorageSnapshot {
        previous: Handle::invalid(),
        operation: unchanged,
    });
    let preserved = flow.control.storage_snapshots.insert(FlowStorageSnapshot {
        previous: observed,
        operation: unchanged,
    });
    let changed = flow.control.storage_snapshots.insert(FlowStorageSnapshot {
        previous: preserved,
        operation: FlowStorageOperation::Unknown,
    });
    let predecessors = flow
        .control
        .storage_predecessors
        .insert_many([preserved, changed]);
    let joined = flow.control.storage_snapshots.insert(FlowStorageSnapshot {
        previous: Handle::invalid(),
        operation: FlowStorageOperation::Join { predecessors },
    });
    let context = crate::frozen_program::CheckingContext::new(&program)
        .clone_with_storage_graph(Arc::new(flow));
    let mut judge = StructuralJudge::from_declaration_requires(
        &context,
        SymbolHandle::invalid(),
        &[],
        &[],
        false,
    );
    judge.storage_version = Some(StorageVersion {
        state,
        snapshot: observed,
    });
    judge.retain_storage_value(
        coordinate.clone(),
        super::super::super::StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(17)),
        false,
    );
    let stored = judge.stored_values.first().unwrap();
    for snapshot in [observed, preserved] {
        assert!(judge.current_value_valid(stored, &coordinate, StorageVersion { state, snapshot }));
    }
    for snapshot in [changed, joined] {
        assert!(!judge.current_value_valid(
            stored,
            &coordinate,
            StorageVersion { state, snapshot }
        ));
    }
}
