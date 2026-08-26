use crate::build_target_operation_plan;
use omega_abstract_operations::{
    AbstractBoundaryEdge, AbstractBoundaryLink, AbstractBoundaryPolicyVerdict,
    AbstractFunctionPlan, AbstractOperationPlan, AbstractPermissionEvent,
    AbstractSourceBoundaryEdge, AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole,
    BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin, BoundaryFootprintPlan,
    CallbackBoundaryFootprintPlan,
};
use omega_calling_conventions::{
    HostCapability, HostOperation, HostOperationKey, MachineRegister, MachineStateSet, RegisterSet,
    StateFootprintEvidence, build_host_abi_plan,
};
use omega_control_flow::{MachineFunctionIdentity, StateKey};
use omega_platform_interface::HostCallPlan;
use omega_target::NativeTarget;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[test]
fn preserves_generated_function_identity_in_target_plan() {
    let continuation = StateKey {
        machine: SymbolHandle::from_arena_index(1),
        state: SymbolHandle::from_arena_index(2),
        segment_index: 0,
    };
    let identity = MachineFunctionIdentity::program_storage_entry_wrapper(continuation)
        .expect("valid continuation should admit wrapper identity");
    let mut abstract_operations = AbstractOperationPlan::default();
    abstract_operations
        .code
        .functions
        .insert(AbstractFunctionPlan {
            symbol: Arc::from("__omega_program_storage_entry"),
            identity,
            instructions: Default::default(),
        });

    let target_operations = build_target_operation_plan(
        NativeTarget::host(),
        &build_host_abi_plan(NativeTarget::host()),
        &HostCallPlan::default(),
        &abstract_operations,
    );

    let [function] = target_operations.code.functions.storage_slice() else {
        panic!("one abstract function should produce one target function")
    };
    assert_eq!(function.identity, identity);
    assert_eq!(function.identity.source_key(), None);
    assert_eq!(
        function.identity.program_storage_entry_continuation(),
        Some(continuation)
    );
}

#[test]
fn preserves_outgoing_stack_address_recipe_in_target_plan() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let instructions = abstract_operations.code.instructions.insert_many([
        omega_abstract_operations::AbstractOperation {
            kind: omega_abstract_operations::AbstractOperationKind::ReserveOutgoingStackFrame {
                byte_count: 72,
            },
            ..Default::default()
        },
        omega_abstract_operations::AbstractOperation {
            kind: omega_abstract_operations::AbstractOperationKind::WriteOutgoingStackU64 {
                stack_byte_offset: 32,
                value: 0x1020,
            },
            ..Default::default()
        },
        omega_abstract_operations::AbstractOperation {
            kind: omega_abstract_operations::AbstractOperationKind::LoadOutgoingStackAddress {
                register: MachineRegister::X86Rcx,
                stack_byte_offset: 32,
            },
            ..Default::default()
        },
        omega_abstract_operations::AbstractOperation {
            kind: omega_abstract_operations::AbstractOperationKind::ReleaseOutgoingStackFrame {
                byte_count: 72,
            },
            ..Default::default()
        },
    ]);
    abstract_operations
        .code
        .functions
        .insert(AbstractFunctionPlan {
            symbol: Arc::from("synthetic_wrapper"),
            identity: MachineFunctionIdentity::default(),
            instructions,
        });
    let target = NativeTarget::uefi_x64();
    let target_operations = build_target_operation_plan(
        target,
        &build_host_abi_plan(target),
        &HostCallPlan::default(),
        &abstract_operations,
    );
    let [reserve, write, instruction, release] =
        target_operations.code.instructions.storage_slice()
    else {
        panic!("balanced caller-frame recipes should survive lowering")
    };
    assert_eq!(
        reserve.kind,
        omega_target_operations::TargetOperationKind::ReserveOutgoingStackFrame { byte_count: 72 }
    );
    assert_eq!(
        write.kind,
        omega_target_operations::TargetOperationKind::WriteOutgoingStackU64 {
            stack_byte_offset: 32,
            value: 0x1020,
        }
    );
    assert_eq!(
        instruction.kind,
        omega_target_operations::TargetOperationKind::LoadOutgoingStackAddress {
            register: MachineRegister::X86Rcx,
            stack_byte_offset: 32,
        }
    );
    assert_eq!(
        release.kind,
        omega_target_operations::TargetOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 }
    );
}

#[test]
fn preserves_single_word_data_address_write_in_target_plan() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let data = omega_abstract_operations::AbstractDataObjectHandle::from_arena_index(4);
    let instructions = abstract_operations.code.instructions.insert_many([
        omega_abstract_operations::AbstractOperation {
            kind: omega_abstract_operations::AbstractOperationKind::WritePlaceAddress {
                source: omega_abstract_operations::Place::at(
                    omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    24,
                ),
                target_offset: 80,
            },
            ..Default::default()
        },
        omega_abstract_operations::AbstractOperation {
            kind:
                omega_abstract_operations::AbstractOperationKind::WriteDataAddressToRuntimeFrame {
                    data,
                    target_offset: 88,
                },
            ..Default::default()
        },
    ]);
    abstract_operations
        .code
        .functions
        .insert(AbstractFunctionPlan {
            symbol: Arc::from("dynamic_descriptor_fixture"),
            identity: MachineFunctionIdentity::default(),
            instructions,
        });

    let target = NativeTarget::linux_arm64();
    let target_operations = build_target_operation_plan(
        target,
        &build_host_abi_plan(target),
        &HostCallPlan::default(),
        &abstract_operations,
    );
    let [instance, table] = target_operations.code.instructions.storage_slice() else {
        panic!("both dynamic descriptor word writes must survive target lowering")
    };
    assert!(matches!(
        instance.kind,
        omega_target_operations::TargetOperationKind::WritePlaceAddress {
            target_offset: 80,
            ..
        }
    ));
    assert_eq!(
        table.kind,
        omega_target_operations::TargetOperationKind::WriteDataAddressToRuntimeFrame {
            data: omega_target_operations::TargetDataObjectHandle::from_arena_index(4),
            target_offset: 88,
        }
    );
}

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
            arithmetic_policy_adapter: None,
            operator_provider_plan_identity: None,
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
        .boundaries
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
    abstract_operations
        .semantics
        .boundaries
        .footprints
        .boundary_contract_fingerprint = Some(0x1234);
    abstract_operations
        .semantics
        .boundaries
        .footprints
        .fragments
        .push(BoundaryFootprintFragment {
            origin: BoundaryFootprintFragmentOrigin::EntryStorage,
            evidence: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86R15]),
                MachineStateSet::empty(),
            ),
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
    abstract_operations
        .semantics
        .boundaries
        .callback_footprints
        .push(CallbackBoundaryFootprintPlan {
            placement_index: 0,
            function_identity: callback_identity,
            footprints: BoundaryFootprintPlan {
                boundary_contract_fingerprint: Some(0x5678),
                ..Default::default()
            },
        });

    let target_operations = build_target_operation_plan(
        NativeTarget::host(),
        &build_host_abi_plan(NativeTarget::host()),
        &HostCallPlan::default(),
        &abstract_operations,
    );

    assert_eq!(target_operations.semantics.boundaries.source_edges.len(), 1);
    let edge = target_operations
        .semantics
        .boundaries
        .source_edges
        .iter()
        .next()
        .map(|(_, edge)| edge)
        .expect("target source boundary edge");
    assert_eq!(edge.statement_index, 9);
    assert_eq!(edge.call_ordinal, 1);
    assert_eq!(edge.boundary_trait_symbol, trait_symbol);
    assert_eq!(edge.boundary_signature_symbol, signature_symbol);
    assert_eq!(
        target_operations
            .semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint,
        Some(0x1234)
    );
    assert_eq!(
        target_operations.semantics.boundaries.footprints.fragments[0].origin,
        BoundaryFootprintFragmentOrigin::EntryStorage
    );
    let [callback] = target_operations
        .semantics
        .boundaries
        .callback_footprints
        .as_slice()
    else {
        panic!("one target callback footprint")
    };
    assert_eq!(callback.placement_index, 0);
    assert_eq!(callback.function_identity, callback_identity);
    assert_eq!(
        callback.footprints.boundary_contract_fingerprint,
        Some(0x5678)
    );
}

#[test]
fn validates_linked_boundary_operation_against_host_binding() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let source_edge = abstract_operations
        .semantics
        .boundaries
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
            .boundaries
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
        .boundaries
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
        .boundaries
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
            .boundaries
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
        .boundaries
        .policy_checks
        .iter()
        .next()
        .map(|(_, check)| check)
        .expect("boundary policy check");
    assert_eq!(
        target_operations.semantics.boundaries.policy_checks.len(),
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
        .boundaries
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
            .boundaries
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
        .boundaries
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
        .boundaries
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
        .boundaries
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
            .boundaries
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
        .boundaries
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
        .boundaries
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
fn copies_abstract_permission_summary_to_target_plan() {
    let mut abstract_operations = AbstractOperationPlan::default();
    let target_symbol = SymbolHandle::from_arena_index(1);
    abstract_operations
        .semantics
        .ownership
        .permissions
        .insert(AbstractPermissionEvent {
            source: psi_language_semantics::PermissionEventSource::Call {
                statement_index: 7,
                call_ordinal: 2,
                target_symbol,
            },
            ..AbstractPermissionEvent::default()
        });

    let target_operations = build_target_operation_plan(
        NativeTarget::host(),
        &build_host_abi_plan(NativeTarget::host()),
        &HostCallPlan::default(),
        &abstract_operations,
    );

    assert_eq!(target_operations.semantics.ownership.permissions.len(), 1);
    let event = target_operations
        .semantics
        .ownership
        .permissions
        .iter()
        .next()
        .map(|(_, event)| event)
        .expect("target ownership event");
    assert_eq!(
        event.source,
        psi_language_semantics::PermissionEventSource::Call {
            statement_index: 7,
            call_ordinal: 2,
            target_symbol,
        }
    );
}
