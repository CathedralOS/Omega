use crate::build_machine_instructions;
use omega_abstract_operations::{
    AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, AbstractPermissionEvent,
    AbstractSourceBoundaryEdge, AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole,
};
use omega_assigned_target_operations::{
    AssignedTargetOperationFunction, AssignedTargetOperationPlan,
};
use omega_control_flow::{MachineFunctionIdentity, StateKey};
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[test]
fn preserves_private_function_identity_through_machine_lowering() {
    let mut assigned = AssignedTargetOperationPlan::default();
    let continuation = StateKey {
        machine: SymbolHandle::from_arena_index(1),
        state: SymbolHandle::from_arena_index(2),
        segment_index: 0,
    };
    let identity = MachineFunctionIdentity::callback_thunk(continuation, 7)
        .expect("valid continuation should admit callback identity");
    assigned
        .code
        .functions
        .insert(AssignedTargetOperationFunction {
            symbol: Arc::from("__omega_callback_test"),
            identity,
            instructions: Default::default(),
        });

    let machine = build_machine_instructions(&assigned).expect("machine instructions");
    let [function] = machine.code.functions.storage_slice() else {
        panic!("one assigned function should produce one machine function")
    };
    assert_eq!(function.symbol.as_ref(), "__omega_callback_test");
    assert_eq!(function.identity, identity);
    assert_eq!(function.identity.source_key(), None);
    assert_eq!(function.identity.callback_thunk_placement_index(), Some(7));
    assert_eq!(
        function.identity.associated_source_continuation(),
        continuation
    );
}

#[test]
fn rejects_invalid_or_aliased_function_identity_before_machine_lowering() {
    let mut invalid = AssignedTargetOperationPlan::default();
    invalid
        .code
        .functions
        .insert(AssignedTargetOperationFunction {
            symbol: Arc::from("invalid"),
            identity: MachineFunctionIdentity::default(),
            instructions: Default::default(),
        });
    let diagnostic = build_machine_instructions(&invalid)
        .expect_err("an invalid function role must not reach machine instructions");
    assert!(
        diagnostic
            .message
            .contains("invalid compiler-private identity")
    );

    let continuation = StateKey {
        machine: SymbolHandle::from_arena_index(1),
        state: SymbolHandle::from_arena_index(2),
        segment_index: 0,
    };
    let identity = MachineFunctionIdentity::callback_thunk(continuation, 7).unwrap();
    let mut duplicate = AssignedTargetOperationPlan::default();
    let mut second = None;
    for symbol in ["callback_a", "callback_b"] {
        let handle = duplicate
            .code
            .functions
            .insert(AssignedTargetOperationFunction {
                symbol: Arc::from(symbol),
                identity,
                instructions: Default::default(),
            });
        second = Some(handle);
    }
    let diagnostic = build_machine_instructions(&duplicate)
        .expect_err("one callback role must not lower to two machine functions");
    assert!(
        diagnostic
            .message
            .contains("share compiler-private identity")
    );
    assert!(diagnostic.message.contains("callback_a"));
    assert!(diagnostic.message.contains("callback_b"));

    duplicate
        .code
        .functions
        .get_mut(second.expect("second callback function"))
        .identity = MachineFunctionIdentity::callback_thunk(continuation, 8).unwrap();
    let machine = build_machine_instructions(&duplicate)
        .expect("distinct placement roles for one continuation must remain distinct");
    assert_eq!(machine.code.functions.len(), 2);
}

#[test]
fn internal_call_target_must_resolve_to_one_exact_assigned_function_role() {
    use omega_assigned_target_operations::{AssignedOperation, AssignedOperationKind};

    let continuation = StateKey {
        machine: SymbolHandle::from_parts(1, 1),
        state: SymbolHandle::from_parts(2, 1),
        segment_index: 0,
    };
    let source_identity = MachineFunctionIdentity::source(continuation);
    let callback_identity = MachineFunctionIdentity::callback_thunk(continuation, 7).unwrap();
    let mut assigned = AssignedTargetOperationPlan::default();
    let instructions = assigned.code.instructions.insert_many([AssignedOperation {
        kind: AssignedOperationKind::CallInternalFunction {
            target: source_identity,
        },
        ..Default::default()
    }]);
    assigned
        .code
        .functions
        .insert(AssignedTargetOperationFunction {
            symbol: Arc::from("__omega_callback_test"),
            identity: callback_identity,
            instructions,
        });
    assigned
        .code
        .functions
        .insert(AssignedTargetOperationFunction {
            symbol: Arc::from("selected_source_entry"),
            identity: source_identity,
            instructions: Default::default(),
        });

    let machine = build_machine_instructions(&assigned)
        .expect("callback thunk may call its exact selected source entry");
    assert_eq!(machine.code.functions.len(), 2);
    assert_eq!(
        machine.code.instructions.storage_slice()[0].source_kind,
        AssignedOperationKind::CallInternalFunction {
            target: source_identity
        }
    );

    assigned
        .code
        .instructions
        .get_mut(instructions.start())
        .kind = AssignedOperationKind::CallInternalFunction {
        target: MachineFunctionIdentity::callback_thunk(continuation, 8).unwrap(),
    };
    let diagnostic = build_machine_instructions(&assigned)
        .expect_err("a missing callback role must not alias the selected source entry");
    assert!(
        diagnostic
            .message
            .contains("calls missing compiler-private function identity")
    );

    assigned
        .code
        .instructions
        .get_mut(instructions.start())
        .kind = AssignedOperationKind::CallInternalFunction {
        target: MachineFunctionIdentity::source(StateKey {
            state: SymbolHandle::from_parts(2, 2),
            ..continuation
        }),
    };
    let diagnostic = build_machine_instructions(&assigned)
        .expect_err("a stale source-entry generation must not resolve");
    assert!(diagnostic.message.contains("__omega_callback_test"));
    assert!(diagnostic.message.contains("generation: 2"));
}

#[test]
fn lowers_outgoing_stack_address_to_exact_machine_kind() {
    let mut assigned = AssignedTargetOperationPlan::default();
    assigned.target = omega_target::NativeTarget::uefi_x64();
    let instructions = assigned.code.instructions.insert_many([
        omega_assigned_target_operations::AssignedOperation {
            kind:
                omega_assigned_target_operations::AssignedOperationKind::ReserveOutgoingStackFrame {
                    byte_count: 72,
                },
            ..Default::default()
        },
        omega_assigned_target_operations::AssignedOperation {
            kind: omega_assigned_target_operations::AssignedOperationKind::WriteOutgoingStackU64 {
                stack_byte_offset: 32,
                value: 0x1000,
            },
            ..Default::default()
        },
        omega_assigned_target_operations::AssignedOperation {
            kind: omega_assigned_target_operations::AssignedOperationKind::WriteOutgoingStackU64 {
                stack_byte_offset: 40,
                value: 0x800,
            },
            ..Default::default()
        },
        omega_assigned_target_operations::AssignedOperation {
            kind: omega_assigned_target_operations::AssignedOperationKind::WriteOutgoingStackU64 {
                stack_byte_offset: 48,
                value: 0x8000,
            },
            ..Default::default()
        },
        omega_assigned_target_operations::AssignedOperation {
            kind: omega_assigned_target_operations::AssignedOperationKind::WriteOutgoingStackU64 {
                stack_byte_offset: 56,
                value: 0x2000,
            },
            ..Default::default()
        },
        omega_assigned_target_operations::AssignedOperation {
            kind:
                omega_assigned_target_operations::AssignedOperationKind::LoadOutgoingStackAddress {
                    register: omega_calling_conventions::MachineRegister::X86Rcx,
                    stack_byte_offset: 32,
                },
            ..Default::default()
        },
        omega_assigned_target_operations::AssignedOperation {
            kind:
                omega_assigned_target_operations::AssignedOperationKind::LoadOutgoingStackAddress {
                    register: omega_calling_conventions::MachineRegister::X86Rdx,
                    stack_byte_offset: 48,
                },
            ..Default::default()
        },
        omega_assigned_target_operations::AssignedOperation {
            kind:
                omega_assigned_target_operations::AssignedOperationKind::ReleaseOutgoingStackFrame {
                    byte_count: 72,
                },
            ..Default::default()
        },
    ]);
    assigned
        .code
        .functions
        .insert(AssignedTargetOperationFunction {
            symbol: Arc::from("synthetic_wrapper"),
            identity: MachineFunctionIdentity::source(StateKey {
                machine: SymbolHandle::from_arena_index(1),
                state: SymbolHandle::from_arena_index(3),
                segment_index: 0,
            }),
            instructions,
        });
    let machine = build_machine_instructions(&assigned).expect("machine lowering");
    let [
        reserve,
        write0,
        write1,
        write2,
        write3,
        address0,
        address1,
        release,
    ] = machine.code.instructions.storage_slice()
    else {
        panic!("balanced caller-frame address instructions should lower")
    };
    assert_eq!(
        reserve.kind,
        omega_machine_instructions::MachineInstructionKind::OutgoingStackFrameReserve
    );
    assert_eq!(
        write0.kind,
        omega_machine_instructions::MachineInstructionKind::OutgoingStackU64Write
    );
    assert_eq!(write1.kind, write0.kind);
    assert_eq!(write2.kind, write0.kind);
    assert_eq!(write3.kind, write0.kind);
    assert_eq!(
        address0.kind,
        omega_machine_instructions::MachineInstructionKind::OutgoingStackAddressLoad
    );
    assert_eq!(
        address1.source_kind,
        omega_assigned_target_operations::AssignedOperationKind::LoadOutgoingStackAddress {
            register: omega_calling_conventions::MachineRegister::X86Rdx,
            stack_byte_offset: 48,
        }
    );
    assert_eq!(
        release.kind,
        omega_machine_instructions::MachineInstructionKind::OutgoingStackFrameRelease
    );
}

#[test]
fn lowers_exact_entry_indirect_copies_to_machine_kinds() {
    use omega_assigned_target_operations::{AssignedOperation, AssignedOperationKind};
    use omega_calling_conventions::MachineRegister;
    use omega_machine_instructions::MachineInstructionKind;

    let mut assigned = AssignedTargetOperationPlan::default();
    assigned.target = omega_target::NativeTarget::uefi_x64();
    let mut rows = vec![AssignedOperation {
        kind: AssignedOperationKind::ReserveOutgoingStackFrame { byte_count: 72 },
        ..Default::default()
    }];
    rows.extend(
        [
            (MachineRegister::X86Rcx, 0, 32),
            (MachineRegister::X86Rcx, 8, 40),
            (MachineRegister::X86Rdx, 0, 48),
            (MachineRegister::X86Rdx, 8, 56),
        ]
        .into_iter()
        .map(
            |(source_register, source_byte_offset, stack_byte_offset)| AssignedOperation {
                kind: AssignedOperationKind::CopyEntryIndirectU64ToOutgoingStack {
                    source_register,
                    source_byte_offset,
                    stack_byte_offset,
                },
                ..Default::default()
            },
        ),
    );
    rows.extend([
        AssignedOperation {
            kind: AssignedOperationKind::LoadOutgoingStackAddress {
                register: MachineRegister::X86Rcx,
                stack_byte_offset: 32,
            },
            ..Default::default()
        },
        AssignedOperation {
            kind: AssignedOperationKind::LoadOutgoingStackAddress {
                register: MachineRegister::X86Rdx,
                stack_byte_offset: 48,
            },
            ..Default::default()
        },
        AssignedOperation {
            kind: AssignedOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 },
            ..Default::default()
        },
    ]);
    let instructions = assigned.code.instructions.insert_many(rows);
    assigned
        .code
        .functions
        .insert(AssignedTargetOperationFunction {
            symbol: Arc::from("synthetic_launch_copy"),
            identity: MachineFunctionIdentity::source(StateKey {
                machine: SymbolHandle::from_arena_index(1),
                state: SymbolHandle::from_arena_index(4),
                segment_index: 0,
            }),
            instructions,
        });

    let machine = build_machine_instructions(&assigned).expect("launch-copy lowering");
    let kinds = machine
        .code
        .instructions
        .iter()
        .map(|(_, instruction)| instruction.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            MachineInstructionKind::OutgoingStackFrameReserve,
            MachineInstructionKind::EntryIndirectU64ToOutgoingStackCopy,
            MachineInstructionKind::EntryIndirectU64ToOutgoingStackCopy,
            MachineInstructionKind::EntryIndirectU64ToOutgoingStackCopy,
            MachineInstructionKind::EntryIndirectU64ToOutgoingStackCopy,
            MachineInstructionKind::OutgoingStackAddressLoad,
            MachineInstructionKind::OutgoingStackAddressLoad,
            MachineInstructionKind::OutgoingStackFrameRelease,
        ]
    );
}

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
            arithmetic_policy_adapter: None,
            operator_provider_plan_identity: None,
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
        .boundaries
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
    assigned_operations
        .semantics
        .boundaries
        .footprints
        .boundary_contract_fingerprint = Some(0x3456);

    let machine_instructions =
        build_machine_instructions(&assigned_operations).expect("machine instructions");

    assert_eq!(
        machine_instructions.semantics.boundaries.source_edges.len(),
        1
    );
    let edge = machine_instructions
        .semantics
        .boundaries
        .source_edges
        .iter()
        .next()
        .map(|(_, edge)| edge)
        .expect("machine boundary edge");
    assert_eq!(edge.statement_index, 12);
    assert_eq!(edge.call_ordinal, 1);
    assert_eq!(edge.boundary_trait_symbol, trait_symbol);
    assert_eq!(edge.boundary_signature_symbol, signature_symbol);
    assert_eq!(
        machine_instructions
            .semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint,
        Some(0x3456)
    );
}

#[test]
fn copies_assigned_permission_summary_to_machine_instruction_plan() {
    let mut assigned_operations = AssignedTargetOperationPlan::default();
    let target_symbol = SymbolHandle::from_arena_index(1);

    assigned_operations
        .semantics
        .ownership
        .permissions
        .insert(AbstractPermissionEvent {
            source: psi_language_semantics::PermissionEventSource::Call {
                statement_index: 13,
                call_ordinal: 2,
                target_symbol,
            },
            ..AbstractPermissionEvent::default()
        });

    let machine_instructions =
        build_machine_instructions(&assigned_operations).expect("machine instructions");

    assert_eq!(
        machine_instructions.semantics.ownership.permissions.len(),
        1
    );
    let event = machine_instructions
        .semantics
        .ownership
        .permissions
        .iter()
        .next()
        .map(|(_, event)| event)
        .expect("machine ownership event");
    assert_eq!(
        event.source,
        psi_language_semantics::PermissionEventSource::Call {
            statement_index: 13,
            call_ordinal: 2,
            target_symbol,
        }
    );
}

#[test]
fn copies_assigned_boundary_policy_checks_to_machine_instruction_plan() {
    let mut assigned_operations = AssignedTargetOperationPlan::default();
    assigned_operations
        .semantics
        .boundaries
        .policy_checks
        .insert(AbstractBoundaryPolicyCheck {
            boundary_policy: "omega::host::targets::linux".into(),
            verdict: AbstractBoundaryPolicyVerdict::MissingSourceBoundary,
            ..Default::default()
        });

    let machine_instructions =
        build_machine_instructions(&assigned_operations).expect("machine instructions");

    let check = machine_instructions
        .semantics
        .boundaries
        .policy_checks
        .iter()
        .next()
        .map(|(_, check)| check)
        .expect("machine boundary policy check");
    assert_eq!(
        machine_instructions
            .semantics
            .boundaries
            .policy_checks
            .len(),
        1
    );
    assert_eq!(
        check.verdict,
        AbstractBoundaryPolicyVerdict::MissingSourceBoundary
    );
    assert_eq!(
        check.boundary_policy.as_ref(),
        "omega::host::targets::linux"
    );
}
