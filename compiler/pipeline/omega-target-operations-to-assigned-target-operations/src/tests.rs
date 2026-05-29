use crate::build_assigned_target_operations;
use omega_abstract_operations::{
    AbstractMoveEvent, AbstractOwnershipEventSource, AbstractValueFact, AbstractValueOrigin,
    AbstractValueStatementRole,
};
use omega_core::symbols::SymbolHandle;
use omega_target_operations::TargetOperationPlan;

#[test]
fn copies_target_value_summary_to_assigned_plan() {
    let mut target_operations = TargetOperationPlan::default();
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);

    target_operations
        .semantics
        .values
        .values
        .insert(AbstractValueFact {
            source_key: Default::default(),
            machine_symbol,
            state_symbol,
            expression: Default::default(),
            origin: AbstractValueOrigin::Statement {
                statement_index: 7,
                role: AbstractValueStatementRole::CallArgument,
            },
        });

    let assigned_operations = build_assigned_target_operations(&target_operations);

    assert_eq!(assigned_operations.semantics.values.values.len(), 1);
    let value = assigned_operations
        .semantics
        .values
        .values
        .iter()
        .next()
        .map(|(_, value)| value)
        .expect("assigned value");
    assert_eq!(
        value.origin,
        AbstractValueOrigin::Statement {
            statement_index: 7,
            role: AbstractValueStatementRole::CallArgument,
        }
    );
}

#[test]
fn copies_target_ownership_summary_to_assigned_plan() {
    let mut target_operations = TargetOperationPlan::default();
    let target_symbol = SymbolHandle::from_arena_index(1);

    target_operations
        .semantics
        .ownership
        .moves
        .insert(AbstractMoveEvent {
            source_key: Default::default(),
            source: AbstractOwnershipEventSource::Call {
                statement_index: 9,
                call_ordinal: 3,
                target_symbol,
            },
            root: Default::default(),
            segments: Default::default(),
        });

    let assigned_operations = build_assigned_target_operations(&target_operations);

    assert_eq!(assigned_operations.semantics.ownership.moves.len(), 1);
    let event = assigned_operations
        .semantics
        .ownership
        .moves
        .iter()
        .next()
        .map(|(_, event)| event)
        .expect("assigned ownership event");
    assert_eq!(
        event.source,
        AbstractOwnershipEventSource::Call {
            statement_index: 9,
            call_ordinal: 3,
            target_symbol,
        }
    );
}
