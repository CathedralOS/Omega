use crate::build_assigned_target_operations;
use omega_abstract_operations::{
    AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, AbstractPermissionEvent,
    AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole,
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
fn copies_target_permission_summary_to_assigned_plan() {
    let mut target_operations = TargetOperationPlan::default();
    let target_symbol = SymbolHandle::from_arena_index(1);

    target_operations
        .semantics
        .ownership
        .permissions
        .insert(AbstractPermissionEvent {
            source: omega_core::semantics::PermissionEventSource::Call {
                statement_index: 9,
                call_ordinal: 3,
                target_symbol,
            },
            ..AbstractPermissionEvent::default()
        });

    let assigned_operations = build_assigned_target_operations(&target_operations);

    assert_eq!(assigned_operations.semantics.ownership.permissions.len(), 1);
    let event = assigned_operations
        .semantics
        .ownership
        .permissions
        .iter()
        .next()
        .map(|(_, event)| event)
        .expect("assigned ownership event");
    assert_eq!(
        event.source,
        omega_core::semantics::PermissionEventSource::Call {
            statement_index: 9,
            call_ordinal: 3,
            target_symbol,
        }
    );
}

#[test]
fn copies_target_boundary_policy_checks_to_assigned_plan() {
    let mut target_operations = TargetOperationPlan::default();
    target_operations
        .semantics
        .boundaries
        .policy_checks
        .insert(AbstractBoundaryPolicyCheck {
            boundary_policy: "omega::host::targets::linux".into(),
            verdict: AbstractBoundaryPolicyVerdict::Accepted,
            ..Default::default()
        });

    let assigned_operations = build_assigned_target_operations(&target_operations);

    let check = assigned_operations
        .semantics
        .boundaries
        .policy_checks
        .iter()
        .next()
        .map(|(_, check)| check)
        .expect("assigned boundary policy check");
    assert_eq!(
        assigned_operations.semantics.boundaries.policy_checks.len(),
        1
    );
    assert_eq!(check.verdict, AbstractBoundaryPolicyVerdict::Accepted);
    assert_eq!(
        check.boundary_policy.as_ref(),
        "omega::host::targets::linux"
    );
}
