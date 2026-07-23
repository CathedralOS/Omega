use crate::build_machine_instructions;
use omega_abstract_operations::{
    AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, AbstractPermissionEvent,
    AbstractSourceBoundaryEdge, AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole,
};
use omega_assigned_target_operations::{
    AssignedOperation, AssignedTargetOperationFunction, AssignedTargetOperationPlan,
    SelectedInstructionKind,
};
use omega_core::symbols::SymbolHandle;

#[test]
fn generated_idt_load_retains_prepared_facts_in_machine_lowering() {
    let mut assigned_operations = AssignedTargetOperationPlan::default();
    let source_kind = SelectedInstructionKind::GeneratedIdtLoad {
        materialized: omega_external_roots::MaterializedIdtId::from_normalized_identity(1)
            .expect("materialized IDT identity"),
        descriptor: omega_external_roots::IdtDestinationId::from_normalized_identity(2)
            .expect("IDT destination identity"),
        content_fingerprint: 3,
        root_ledger_fingerprint: 4,
        control: omega_external_roots::IdtControlId::from_normalized_identity(5)
            .expect("IDT control identity"),
    };
    let instructions = assigned_operations
        .code
        .instructions
        .insert_many([AssignedOperation {
            kind: source_kind.clone(),
            source_key: Default::default(),
            source_statement: 0,
        }]);
    assigned_operations
        .code
        .functions
        .insert(AssignedTargetOperationFunction {
            instructions,
            ..Default::default()
        });

    let machine = build_machine_instructions(&assigned_operations)
        .expect("generated IDT operation should lower to machine carrier");
    let instruction = machine
        .code
        .instructions
        .iter()
        .next()
        .map(|(_, instruction)| instruction)
        .expect("generated machine instruction");
    assert_eq!(
        instruction.kind,
        omega_machine_instructions::MachineInstructionKind::GeneratedIdtLoad
    );
    assert_eq!(instruction.source_kind, source_kind);
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
            source: omega_core::semantics::PermissionEventSource::Call {
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
        omega_core::semantics::PermissionEventSource::Call {
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
