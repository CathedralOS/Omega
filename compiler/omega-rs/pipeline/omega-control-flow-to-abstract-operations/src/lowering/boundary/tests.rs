use super::*;
use omega_calling_conventions::{HostCapability, HostOperation, HostOperationKey};
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_platform_interface::{HostCall, HostCallPlan, LoweredHostOperation};
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[test]
fn copies_host_operations_as_boundary_edges() {
    let mut host_calls = HostCallPlan::default();
    let mut call = HostCall {
        source_key: StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        },
        statement_index: 5,
        call_ordinal: 2,
        ..HostCall::default()
    };
    let operation_key = HostOperationKey::new(HostCapability::Stdout, HostOperation::Write);
    host_calls.operations.append_to_span(
        &mut call.operations,
        LoweredHostOperation {
            operation_key,
            fixed_leading_immediate: None,
        },
    );
    host_calls.operations.append_to_span(
        &mut call.operations,
        LoweredHostOperation {
            operation_key: HostOperationKey::new(HostCapability::Stdout, HostOperation::WriteFile),
            fixed_leading_immediate: None,
        },
    );
    call.arguments = HandleSpan::empty();
    host_calls.calls.insert(call);

    let summary = build_abstract_boundary_summary(&ControlFlowPlan::default(), &host_calls);

    let edges: Vec<_> = summary.edges.iter().map(|(_, edge)| edge).collect();
    assert_eq!(summary.edges.len(), 2);
    assert_eq!(edges[0].statement_index, 5);
    assert_eq!(edges[0].call_ordinal, 2);
    assert_eq!(edges[0].operation_ordinal, 0);
    assert_eq!(edges[0].operation_key, operation_key);
    assert_eq!(edges[1].operation_ordinal, 1);
}

#[test]
fn copies_control_flow_boundary_edges_as_source_boundary_edges() {
    let mut control_flow = ControlFlowPlan::default();
    let state_key = StateKey {
        machine: SymbolHandle::from_arena_index(1),
        state: SymbolHandle::from_arena_index(2),
        segment_index: 0,
    };
    let mut edge_span = HandleSpan::empty();
    control_flow.semantics.boundaries.edges.append_to_span(
        &mut edge_span,
        omega_control_flow::StateBoundaryEdge {
            statement_index: 8,
            call_ordinal: 1,
            receiver_symbol: SymbolHandle::from_arena_index(3),
            target_symbol: SymbolHandle::from_arena_index(4),
            boundary_trait_symbol: SymbolHandle::from_arena_index(5),
            boundary_signature_symbol: SymbolHandle::from_arena_index(6),
        },
    );
    control_flow.states.insert(omega_control_flow::StateFlow {
        key: state_key,
        boundaries: omega_control_flow::StateBoundarySummary { edges: edge_span },
        ..Default::default()
    });

    let summary = build_abstract_boundary_summary(&control_flow, &HostCallPlan::default());

    let edge = summary
        .source_edges
        .iter()
        .next()
        .map(|(_, edge)| edge)
        .unwrap();
    assert_eq!(summary.source_edges.len(), 1);
    assert_eq!(edge.source_key, state_key);
    assert_eq!(edge.statement_index, 8);
    assert_eq!(edge.call_ordinal, 1);
    assert_eq!(
        edge.boundary_trait_symbol,
        SymbolHandle::from_arena_index(5)
    );
    assert_eq!(
        edge.boundary_signature_symbol,
        SymbolHandle::from_arena_index(6)
    );
}

#[test]
fn links_source_boundary_edges_to_lowered_host_operations() {
    let control_flow = control_flow_with_source_boundary_edge();
    let state_key = control_flow
        .states
        .iter()
        .next()
        .map(|(_, state)| state.key)
        .unwrap();

    let mut host_calls = HostCallPlan::default();
    let mut call = HostCall {
        source_key: state_key,
        statement_index: 8,
        call_ordinal: 1,
        ..HostCall::default()
    };
    let operation_key = HostOperationKey::new(HostCapability::Stdout, HostOperation::Write);
    host_calls.operations.append_to_span(
        &mut call.operations,
        LoweredHostOperation {
            operation_key,
            fixed_leading_immediate: None,
        },
    );
    host_calls.calls.insert(call);

    let summary = build_abstract_boundary_summary(&control_flow, &host_calls);

    assert_eq!(summary.source_edges.len(), 1);
    assert_eq!(summary.edges.len(), 1);
    let link = summary.links.iter().next().map(|(_, link)| link).unwrap();
    assert_eq!(summary.links.len(), 1);
    assert_eq!(
        summary
            .source_edges
            .get(link.source_edge)
            .boundary_signature_symbol,
        SymbolHandle::from_arena_index(6)
    );
    assert_eq!(
        summary.edges.get(link.lowered_edge).operation_key,
        operation_key
    );
}

#[test]
fn does_not_link_distinct_call_ordinals_on_same_statement() {
    let control_flow = control_flow_with_source_boundary_edge();
    let state_key = control_flow
        .states
        .iter()
        .next()
        .map(|(_, state)| state.key)
        .unwrap();

    let mut host_calls = HostCallPlan::default();
    let mut call = HostCall {
        source_key: state_key,
        statement_index: 8,
        call_ordinal: 2,
        ..HostCall::default()
    };
    host_calls.operations.append_to_span(
        &mut call.operations,
        LoweredHostOperation {
            operation_key: HostOperationKey::new(HostCapability::Stdout, HostOperation::Write),
            fixed_leading_immediate: None,
        },
    );
    host_calls.calls.insert(call);

    let summary = build_abstract_boundary_summary(&control_flow, &host_calls);

    assert_eq!(summary.source_edges.len(), 1);
    assert_eq!(summary.edges.len(), 1);
    assert_eq!(summary.links.len(), 0);
}

fn control_flow_with_source_boundary_edge() -> ControlFlowPlan {
    let mut control_flow = ControlFlowPlan::default();
    let state_key = StateKey {
        machine: SymbolHandle::from_arena_index(1),
        state: SymbolHandle::from_arena_index(2),
        segment_index: 0,
    };
    let mut edge_span = HandleSpan::empty();
    control_flow.semantics.boundaries.edges.append_to_span(
        &mut edge_span,
        omega_control_flow::StateBoundaryEdge {
            statement_index: 8,
            call_ordinal: 1,
            receiver_symbol: SymbolHandle::from_arena_index(3),
            target_symbol: SymbolHandle::from_arena_index(4),
            boundary_trait_symbol: SymbolHandle::from_arena_index(5),
            boundary_signature_symbol: SymbolHandle::from_arena_index(6),
        },
    );
    control_flow.states.insert(omega_control_flow::StateFlow {
        key: state_key,
        boundaries: omega_control_flow::StateBoundarySummary { edges: edge_span },
        ..Default::default()
    });
    control_flow
}
