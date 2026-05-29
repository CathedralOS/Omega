use crate::build_target_operation_plan;
use omega_abstract_operations::{
    AbstractMoveEvent, AbstractOperationPlan, AbstractOwnershipEventSource,
    AbstractSourceBoundaryEdge, AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole,
};
use omega_calling_conventions::build_host_abi_plan;
use omega_core::symbols::SymbolHandle;
use omega_platform_interface::HostCallPlan;
use omega_target::NativeTarget;

#[test]
fn copies_abstract_value_summary_to_target_plan() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);

    abstract_operations.values.values.insert(AbstractValueFact {
        source_key: Default::default(),
        machine_symbol,
        state_symbol,
        expression: Default::default(),
        origin: AbstractValueOrigin::Statement {
            statement_index: 5,
            role: AbstractValueStatementRole::AssignmentValue,
        },
    });

    let target_operations = build_target_operation_plan(
        NativeTarget::host(),
        &build_host_abi_plan(NativeTarget::host()),
        &HostCallPlan::default(),
        &abstract_operations,
    );

    assert_eq!(target_operations.values.values.len(), 1);
    let value = target_operations
        .values
        .values
        .iter()
        .next()
        .map(|(_, value)| value)
        .expect("target value");
    assert_eq!(
        value.origin,
        AbstractValueOrigin::Statement {
            statement_index: 5,
            role: AbstractValueStatementRole::AssignmentValue,
        }
    );
}

#[test]
fn copies_abstract_source_boundary_edges_to_target_plan() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let trait_symbol = SymbolHandle::from_arena_index(3);
    let signature_symbol = SymbolHandle::from_arena_index(4);

    abstract_operations
        .boundary_edges
        .source_edges
        .insert(AbstractSourceBoundaryEdge {
            source_key: Default::default(),
            statement_index: 9,
            call_ordinal: 1,
            receiver_symbol: machine_symbol,
            target_symbol: state_symbol,
            boundary_trait_symbol: trait_symbol,
            boundary_signature_symbol: signature_symbol,
        });

    let target_operations = build_target_operation_plan(
        NativeTarget::host(),
        &build_host_abi_plan(NativeTarget::host()),
        &HostCallPlan::default(),
        &abstract_operations,
    );

    assert_eq!(target_operations.boundary_edges.source_edges.len(), 1);
    let edge = target_operations
        .boundary_edges
        .source_edges
        .iter()
        .next()
        .map(|(_, edge)| edge)
        .expect("target source boundary edge");
    assert_eq!(edge.statement_index, 9);
    assert_eq!(edge.call_ordinal, 1);
    assert_eq!(edge.boundary_trait_symbol, trait_symbol);
    assert_eq!(edge.boundary_signature_symbol, signature_symbol);
}

#[test]
fn copies_abstract_ownership_summary_to_target_plan() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let target_symbol = SymbolHandle::from_arena_index(1);
    abstract_operations
        .ownership
        .moves
        .insert(AbstractMoveEvent {
            source_key: Default::default(),
            source: AbstractOwnershipEventSource::Call {
                statement_index: 7,
                call_ordinal: 2,
                target_symbol,
            },
            root: Default::default(),
            segments: Default::default(),
        });

    let target_operations = build_target_operation_plan(
        NativeTarget::host(),
        &build_host_abi_plan(NativeTarget::host()),
        &HostCallPlan::default(),
        &abstract_operations,
    );

    assert_eq!(target_operations.ownership.moves.len(), 1);
    let event = target_operations
        .ownership
        .moves
        .iter()
        .next()
        .map(|(_, event)| event)
        .expect("target ownership event");
    assert_eq!(
        event.source,
        AbstractOwnershipEventSource::Call {
            statement_index: 7,
            call_ordinal: 2,
            target_symbol,
        }
    );
}
