use crate::build_target_operation_plan;
use omega_abstract_operations::{
    AbstractBoundaryEdge, AbstractBoundaryLink, AbstractBoundaryPolicyVerdict, AbstractMoveEvent,
    AbstractOperationPlan, AbstractOwnershipEventSource, AbstractSourceBoundaryEdge,
    AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole,
};
use omega_calling_conventions::{
    HostCapability, HostOperation, HostOperationKey, build_host_abi_plan,
};
use omega_core::symbols::SymbolHandle;
use omega_platform_interface::HostCallPlan;
use omega_target::NativeTarget;

#[test]
fn copies_abstract_value_summary_to_target_plan() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);

    abstract_operations
        .semantics
        .values
        .values
        .insert(AbstractValueFact {
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

    assert_eq!(target_operations.semantics.values.values.len(), 1);
    let value = target_operations
        .semantics
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
        .semantics
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

    assert_eq!(
        target_operations
            .semantics
            .boundary_edges
            .source_edges
            .len(),
        1
    );
    let edge = target_operations
        .semantics
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
fn validates_linked_boundary_operation_against_host_binding() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let source_edge = abstract_operations
        .semantics
        .boundary_edges
        .source_edges
        .insert(AbstractSourceBoundaryEdge {
            source_key: Default::default(),
            statement_index: 9,
            call_ordinal: 1,
            receiver_symbol: SymbolHandle::from_arena_index(1),
            target_symbol: SymbolHandle::from_arena_index(2),
            boundary_trait_symbol: SymbolHandle::from_arena_index(3),
            boundary_signature_symbol: SymbolHandle::from_arena_index(4),
        });
    let operation_key = HostOperationKey::new(HostCapability::Stdout, HostOperation::Write);
    let lowered_edge =
        abstract_operations
            .semantics
            .boundary_edges
            .edges
            .insert(AbstractBoundaryEdge {
                source_key: Default::default(),
                statement_index: 9,
                call_ordinal: 1,
                operation_ordinal: 0,
                operation_key,
            });
    abstract_operations
        .semantics
        .boundary_edges
        .links
        .insert(AbstractBoundaryLink {
            source_edge,
            lowered_edge,
        });

    let target_operations = build_target_operation_plan(
        NativeTarget::linux_arm64(),
        &build_host_abi_plan(NativeTarget::linux_arm64()),
        &HostCallPlan::default(),
        &abstract_operations,
    );

    let checks: Vec<_> = target_operations
        .semantics
        .boundary_edges
        .policy_checks
        .iter()
        .map(|(_, check)| check)
        .collect();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].source_edge, source_edge);
    assert_eq!(checks[0].lowered_edge, lowered_edge);
    assert_eq!(checks[0].operation_key, operation_key);
    assert_eq!(
        checks[0].boundary_policy.as_ref(),
        "omega::host::targets::linux"
    );
    assert_eq!(checks[0].verdict, AbstractBoundaryPolicyVerdict::Accepted);
}

#[test]
fn records_missing_source_boundary_for_unlinked_host_operation() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let operation_key = HostOperationKey::new(HostCapability::Stdout, HostOperation::Write);
    let lowered_edge =
        abstract_operations
            .semantics
            .boundary_edges
            .edges
            .insert(AbstractBoundaryEdge {
                source_key: Default::default(),
                statement_index: 9,
                call_ordinal: 1,
                operation_ordinal: 0,
                operation_key,
            });

    let target_operations = build_target_operation_plan(
        NativeTarget::linux_arm64(),
        &build_host_abi_plan(NativeTarget::linux_arm64()),
        &HostCallPlan::default(),
        &abstract_operations,
    );

    let check = target_operations
        .semantics
        .boundary_edges
        .policy_checks
        .iter()
        .next()
        .map(|(_, check)| check)
        .expect("boundary policy check");
    assert_eq!(
        target_operations
            .semantics
            .boundary_edges
            .policy_checks
            .len(),
        1
    );
    assert!(!check.source_edge.is_valid());
    assert_eq!(check.lowered_edge, lowered_edge);
    assert_eq!(
        check.verdict,
        AbstractBoundaryPolicyVerdict::MissingSourceBoundary
    );
}

#[test]
fn records_missing_host_binding_for_unknown_boundary_operation() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let source_edge = abstract_operations
        .semantics
        .boundary_edges
        .source_edges
        .insert(AbstractSourceBoundaryEdge {
            source_key: Default::default(),
            statement_index: 9,
            call_ordinal: 1,
            receiver_symbol: SymbolHandle::from_arena_index(1),
            target_symbol: SymbolHandle::from_arena_index(2),
            boundary_trait_symbol: SymbolHandle::from_arena_index(3),
            boundary_signature_symbol: SymbolHandle::from_arena_index(4),
        });
    let operation_key = HostOperationKey::new(HostCapability::Unknown, HostOperation::Unknown);
    let lowered_edge =
        abstract_operations
            .semantics
            .boundary_edges
            .edges
            .insert(AbstractBoundaryEdge {
                source_key: Default::default(),
                statement_index: 9,
                call_ordinal: 1,
                operation_ordinal: 0,
                operation_key,
            });
    abstract_operations
        .semantics
        .boundary_edges
        .links
        .insert(AbstractBoundaryLink {
            source_edge,
            lowered_edge,
        });

    let target_operations = build_target_operation_plan(
        NativeTarget::linux_arm64(),
        &build_host_abi_plan(NativeTarget::linux_arm64()),
        &HostCallPlan::default(),
        &abstract_operations,
    );

    let check = target_operations
        .semantics
        .boundary_edges
        .policy_checks
        .iter()
        .next()
        .map(|(_, check)| check)
        .expect("boundary policy check");
    assert_eq!(
        check.verdict,
        AbstractBoundaryPolicyVerdict::MissingHostBinding
    );
    assert!(check.boundary_policy.is_empty());
}

#[test]
fn records_disallowed_boundary_policy_for_unallowed_host_binding_policy() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let source_edge = abstract_operations
        .semantics
        .boundary_edges
        .source_edges
        .insert(AbstractSourceBoundaryEdge {
            source_key: Default::default(),
            statement_index: 9,
            call_ordinal: 1,
            receiver_symbol: SymbolHandle::from_arena_index(1),
            target_symbol: SymbolHandle::from_arena_index(2),
            boundary_trait_symbol: SymbolHandle::from_arena_index(3),
            boundary_signature_symbol: SymbolHandle::from_arena_index(4),
        });
    let operation_key = HostOperationKey::new(HostCapability::Stdout, HostOperation::Write);
    let lowered_edge =
        abstract_operations
            .semantics
            .boundary_edges
            .edges
            .insert(AbstractBoundaryEdge {
                source_key: Default::default(),
                statement_index: 9,
                call_ordinal: 1,
                operation_ordinal: 0,
                operation_key,
            });
    abstract_operations
        .semantics
        .boundary_edges
        .links
        .insert(AbstractBoundaryLink {
            source_edge,
            lowered_edge,
        });
    let mut host_abi = build_host_abi_plan(NativeTarget::linux_arm64());
    host_abi.boundary_policies.clear();

    let target_operations = build_target_operation_plan(
        NativeTarget::linux_arm64(),
        &host_abi,
        &HostCallPlan::default(),
        &abstract_operations,
    );

    let check = target_operations
        .semantics
        .boundary_edges
        .policy_checks
        .iter()
        .next()
        .map(|(_, check)| check)
        .expect("boundary policy check");
    assert_eq!(
        check.verdict,
        AbstractBoundaryPolicyVerdict::DisallowedBoundaryPolicy
    );
    assert_eq!(
        check.boundary_policy.as_ref(),
        "omega::host::targets::linux"
    );
}

#[test]
fn copies_abstract_ownership_summary_to_target_plan() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let target_symbol = SymbolHandle::from_arena_index(1);
    abstract_operations
        .semantics
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

    assert_eq!(target_operations.semantics.ownership.moves.len(), 1);
    let event = target_operations
        .semantics
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
