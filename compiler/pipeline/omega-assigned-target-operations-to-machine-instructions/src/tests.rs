use crate::build_machine_instructions;
use omega_abstract_operations::{
    AbstractMoveEvent, AbstractOwnershipEventSource, AbstractSourceBoundaryEdge, AbstractValueFact,
    AbstractValueOrigin, AbstractValueStatementRole,
};
use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::symbols::SymbolHandle;

#[test]
fn copies_assigned_value_summary_to_machine_instruction_plan() {
    let mut assigned_operations = AssignedTargetOperationPlan::default();
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);

    assigned_operations
        .semantics
        .values
        .values
        .insert(AbstractValueFact {
            source_key: Default::default(),
            machine_symbol,
            state_symbol,
            expression: Default::default(),
            origin: AbstractValueOrigin::Statement {
                statement_index: 11,
                role: AbstractValueStatementRole::TransitionGuard,
            },
        });

    let machine_instructions =
        build_machine_instructions(&assigned_operations).expect("machine instructions");

    assert_eq!(machine_instructions.semantics.values.values.len(), 1);
    let value = machine_instructions
        .semantics
        .values
        .values
        .iter()
        .next()
        .map(|(_, value)| value)
        .expect("machine value");
    assert_eq!(
        value.origin,
        AbstractValueOrigin::Statement {
            statement_index: 11,
            role: AbstractValueStatementRole::TransitionGuard,
        }
    );
}

#[test]
fn copies_assigned_boundary_summary_to_machine_instruction_plan() {
    let mut assigned_operations = AssignedTargetOperationPlan::default();
    let trait_symbol = SymbolHandle::from_arena_index(1);
    let signature_symbol = SymbolHandle::from_arena_index(2);

    assigned_operations
        .semantics
        .boundary_edges
        .source_edges
        .insert(AbstractSourceBoundaryEdge {
            source_key: Default::default(),
            statement_index: 12,
            call_ordinal: 1,
            receiver_symbol: Default::default(),
            target_symbol: Default::default(),
            boundary_trait_symbol: trait_symbol,
            boundary_signature_symbol: signature_symbol,
        });

    let machine_instructions =
        build_machine_instructions(&assigned_operations).expect("machine instructions");

    assert_eq!(
        machine_instructions
            .semantics
            .boundary_edges
            .source_edges
            .len(),
        1
    );
    let edge = machine_instructions
        .semantics
        .boundary_edges
        .source_edges
        .iter()
        .next()
        .map(|(_, edge)| edge)
        .expect("machine boundary edge");
    assert_eq!(edge.statement_index, 12);
    assert_eq!(edge.call_ordinal, 1);
    assert_eq!(edge.boundary_trait_symbol, trait_symbol);
    assert_eq!(edge.boundary_signature_symbol, signature_symbol);
}

#[test]
fn copies_assigned_ownership_summary_to_machine_instruction_plan() {
    let mut assigned_operations = AssignedTargetOperationPlan::default();
    let target_symbol = SymbolHandle::from_arena_index(1);

    assigned_operations
        .semantics
        .ownership
        .moves
        .insert(AbstractMoveEvent {
            source_key: Default::default(),
            source: AbstractOwnershipEventSource::Call {
                statement_index: 13,
                call_ordinal: 2,
                target_symbol,
            },
            root: Default::default(),
            segments: Default::default(),
        });

    let machine_instructions =
        build_machine_instructions(&assigned_operations).expect("machine instructions");

    assert_eq!(machine_instructions.semantics.ownership.moves.len(), 1);
    let event = machine_instructions
        .semantics
        .ownership
        .moves
        .iter()
        .next()
        .map(|(_, event)| event)
        .expect("machine ownership event");
    assert_eq!(
        event.source,
        AbstractOwnershipEventSource::Call {
            statement_index: 13,
            call_ordinal: 2,
            target_symbol,
        }
    );
}
