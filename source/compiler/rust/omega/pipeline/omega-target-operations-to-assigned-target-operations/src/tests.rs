use crate::build_assigned_target_operations;
use omega_abstract_operations::{
    AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, AbstractPermissionEvent,
    AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole, BoundaryFootprintPlan,
    CallbackBoundaryFootprintPlan,
};
use omega_control_flow::{MachineFunctionIdentity, StateKey};
use omega_target_operations::TargetOperationFunction;
use omega_target_operations::TargetOperationPlan;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[test]
fn preserves_generated_function_identity_in_assigned_plan() {
    let continuation = StateKey {
        machine: SymbolHandle::from_arena_index(1),
        state: SymbolHandle::from_arena_index(2),
        segment_index: 0,
    };
    let identity = MachineFunctionIdentity::program_storage_entry_wrapper(continuation)
        .expect("valid continuation should admit wrapper identity");
    let mut target_operations = TargetOperationPlan::default();
    target_operations
        .code
        .functions
        .insert(TargetOperationFunction {
            symbol: Arc::from("__omega_program_storage_entry"),
            identity,
            instructions: Default::default(),
        });

    let assigned_operations = build_assigned_target_operations(&target_operations);

    let [function] = assigned_operations.code.functions.storage_slice() else {
        panic!("one target function should produce one assigned function")
    };
    assert_eq!(function.identity, identity);
    assert_eq!(function.identity.source_key(), None);
    assert_eq!(
        function.identity.program_storage_entry_continuation(),
        Some(continuation)
    );
}

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
            arithmetic_policy_adapter: None,
            operator_provider_plan_identity: None,
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
            source: psi_language_semantics::PermissionEventSource::Call {
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
        psi_language_semantics::PermissionEventSource::Call {
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
        .footprints
        .boundary_contract_fingerprint = Some(0x2345);
    target_operations
        .semantics
        .boundaries
        .policy_checks
        .insert(AbstractBoundaryPolicyCheck {
            boundary_policy: "omega::host::targets::linux".into(),
            verdict: AbstractBoundaryPolicyVerdict::Accepted,
            ..Default::default()
        });
    let callback_identity = MachineFunctionIdentity::callback_thunk(
        StateKey {
            machine: SymbolHandle::from_arena_index(8),
            state: SymbolHandle::from_arena_index(9),
            segment_index: 0,
        },
        0,
    )
    .expect("callback identity");
    target_operations
        .semantics
        .boundaries
        .callback_footprints
        .push(CallbackBoundaryFootprintPlan {
            placement_index: 0,
            function_identity: callback_identity,
            footprints: BoundaryFootprintPlan {
                boundary_contract_fingerprint: Some(0x6789),
                ..Default::default()
            },
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
    assert_eq!(
        assigned_operations
            .semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint,
        Some(0x2345)
    );
    let [callback] = assigned_operations
        .semantics
        .boundaries
        .callback_footprints
        .as_slice()
    else {
        panic!("one assigned callback footprint")
    };
    assert_eq!(callback.function_identity, callback_identity);
    assert_eq!(
        callback.footprints.boundary_contract_fingerprint,
        Some(0x6789)
    );
}
