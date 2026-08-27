use super::*;
use omega_calling_conventions::{
    HostCapability, HostOperation, HostOperationKey, PlatformCallLowering,
    callback_native_parameter_id,
};
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_platform_interface::{
    HostCall, HostCallArgument, HostCallFormalArgumentIdentity, HostCallPlan, LoweredHostOperation,
};
use psi_arena::HandleSpan;
use psi_checked_trees::NominalMachineUseSite;
use psi_checked_trees::statement::StatementHandle;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

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
        ..exact_host_call(SymbolHandle::from_arena_index(9))
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

    let summary = build_abstract_boundary_summary(&ControlFlowPlan::default(), &host_calls)
        .expect("exact boundary summary");

    let edges: Vec<_> = summary.edges.iter().map(|(_, edge)| edge).collect();
    assert_eq!(summary.edges.len(), 2);
    assert_eq!(summary.host_calls.len(), 1);
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

    let summary = build_abstract_boundary_summary(&control_flow, &HostCallPlan::default())
        .expect("source-only boundary summary");

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
        ..exact_host_call(SymbolHandle::from_arena_index(4))
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

    let summary = build_abstract_boundary_summary(&control_flow, &host_calls)
        .expect("linked boundary summary");

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
        ..exact_host_call(SymbolHandle::from_arena_index(4))
    };
    host_calls.operations.append_to_span(
        &mut call.operations,
        LoweredHostOperation {
            operation_key: HostOperationKey::new(HostCapability::Stdout, HostOperation::Write),
            fixed_leading_immediate: None,
        },
    );
    host_calls.calls.insert(call);

    let summary = build_abstract_boundary_summary(&control_flow, &host_calls)
        .expect("unlinked distinct call");

    assert_eq!(summary.source_edges.len(), 1);
    assert_eq!(summary.edges.len(), 1);
    assert_eq!(summary.links.len(), 0);
}

#[test]
fn retains_exact_occurrence_and_ordered_native_formals() {
    let requirement: Arc<str> = Arc::from("package::Registrar::register#exact");
    let mut host_calls = HostCallPlan::default();
    let mut call = exact_host_call(SymbolHandle::from_arena_index(4));
    call.source_site = Some(NominalMachineUseSite::Expression(
        psi_checked_trees::expression::ExpressionHandle::from_arena_index(8),
    ));
    call.requirement_identity = requirement.clone();
    call.has_result = true;
    host_calls.arguments.append_to_span(
        &mut call.arguments,
        HostCallArgument {
            formal: None,
            ..HostCallArgument::default()
        },
    );
    for ordinal in 0..2 {
        host_calls.arguments.append_to_span(
            &mut call.arguments,
            HostCallArgument {
                formal: Some(HostCallFormalArgumentIdentity {
                    formal_ordinal: ordinal,
                    native_parameter: callback_native_parameter_id(&requirement, ordinal),
                }),
                ..HostCallArgument::default()
            },
        );
    }
    host_calls.calls.insert(call.clone());

    let summary = build_abstract_boundary_summary(&ControlFlowPlan::default(), &host_calls)
        .expect("exact host-call occurrence");
    let (_, occurrence) = summary.host_calls.iter().next().expect("occurrence");
    assert_eq!(occurrence.source_site, call_source_site(&call));
    assert_eq!(
        occurrence.registration_operation,
        call.registration_operation
    );
    assert_eq!(occurrence.requirement_identity, requirement);
    let arguments = summary
        .host_call_arguments
        .span(occurrence.arguments)
        .expect("native arguments");
    assert_eq!(arguments.len(), 2, "result pseudo-argument is not a formal");
    assert_eq!(arguments[0].formal_ordinal, 0);
    assert_eq!(arguments[1].formal_ordinal, 1);
    assert_eq!(
        arguments[1].native_parameter,
        Some(callback_native_parameter_id(&requirement, 1))
    );
}

#[test]
fn rejects_native_formal_identity_drift() {
    let mut host_calls = HostCallPlan::default();
    let mut call = exact_host_call(SymbolHandle::from_arena_index(4));
    host_calls.arguments.append_to_span(
        &mut call.arguments,
        HostCallArgument {
            formal: Some(HostCallFormalArgumentIdentity {
                formal_ordinal: 0,
                native_parameter: callback_native_parameter_id("wrong::overload", 0),
            }),
            ..HostCallArgument::default()
        },
    );
    host_calls.calls.insert(call);

    let error = build_abstract_boundary_summary(&ControlFlowPlan::default(), &host_calls)
        .expect_err("native identity drift must fail closed");
    assert!(error.message.contains("native identity"), "{error}");
}

#[test]
fn replay_rejects_occurrence_and_cardinality_drift() {
    let mut host_calls = HostCallPlan::default();
    host_calls
        .calls
        .insert(exact_host_call(SymbolHandle::from_arena_index(4)));
    let summary = build_abstract_boundary_summary(&ControlFlowPlan::default(), &host_calls)
        .expect("exact summary");

    let mut target_drift = summary.clone();
    let occurrence = target_drift.host_calls.iter().next().unwrap().0;
    target_drift
        .host_calls
        .get_mut(occurrence)
        .registration_operation = SymbolHandle::from_arena_index(41);
    assert!(
        validate_abstract_boundary_summary(&host_calls, &target_drift)
            .expect_err("target drift")
            .message
            .contains("identity drift")
    );

    let mut duplicate = summary;
    duplicate
        .host_calls
        .insert(duplicate.host_calls.get(occurrence).clone());
    assert!(
        validate_abstract_boundary_summary(&host_calls, &duplicate)
            .expect_err("duplicate occurrence")
            .message
            .contains("cardinality")
    );
}

#[test]
fn does_not_link_same_coordinates_with_different_registrar_target() {
    let control_flow = control_flow_with_source_boundary_edge();
    let state_key = control_flow.states.iter().next().unwrap().1.key;
    let mut host_calls = HostCallPlan::default();
    let mut call = HostCall {
        source_key: state_key,
        statement_index: 8,
        call_ordinal: 1,
        ..exact_host_call(SymbolHandle::from_arena_index(40))
    };
    host_calls.operations.append_to_span(
        &mut call.operations,
        LoweredHostOperation {
            operation_key: HostOperationKey::new(HostCapability::Stdout, HostOperation::Write),
            fixed_leading_immediate: None,
        },
    );
    host_calls.calls.insert(call);

    let summary = build_abstract_boundary_summary(&control_flow, &host_calls)
        .expect("target-distinct summary");
    assert!(summary.links.is_empty());
}

fn exact_host_call(registration_operation: SymbolHandle) -> HostCall {
    HostCall {
        source_site: Some(NominalMachineUseSite::Statement(
            StatementHandle::from_arena_index(7),
        )),
        registration_operation,
        requirement_identity: Arc::from("package::Registrar::register#exact"),
        lowering: psi_arena::Handle::<PlatformCallLowering>::from_arena_index(3),
        ..HostCall::default()
    }
}

fn call_source_site(call: &HostCall) -> omega_abstract_operations::AbstractHostCallSourceSite {
    match call.source_site.expect("source site") {
        NominalMachineUseSite::Statement(handle) => {
            omega_abstract_operations::AbstractHostCallSourceSite::Statement(handle)
        }
        NominalMachineUseSite::Expression(handle) => {
            omega_abstract_operations::AbstractHostCallSourceSite::Expression(handle)
        }
    }
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
