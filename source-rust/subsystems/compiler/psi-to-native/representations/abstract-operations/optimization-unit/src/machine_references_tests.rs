//! Machine pruning keeps every machine that an operation can transfer control
//! to. The stage 03 proposer finds callees through `call_machine_references`;
//! the independent pruning validator uses `operation_machine_references`. Both
//! must name the callee of every direct call, including a call whose callee
//! never returns normally, or the validator expects a different pruned set and
//! refuses a valid candidate.

use std::sync::Arc;

use super::*;
use terminal_psi::{SemanticFingerprint, VocabularyMarker};

fn unit() -> PsiOptimizationUnit {
    PsiOptimizationUnit {
        identity: OptimizationUnitIdentity::from_canonical_bytes(b"machine reference test"),
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
        },
        work_schedule: WorkScheduleIdentity::new(1).unwrap(),
        scope_provider_closure: None,
        lowering_target: None,
        scalar_values_digest: None,
        entry: MachineId::new(1).unwrap(),
        structural_types: Vec::new().into(),
        structural_domains: Arc::new([]),
        services: Arc::new([]),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        dynamic_dispatch: Default::default(),
        callables: Default::default(),
        accepted_obligation_facts: Vec::new(),
        identities: Default::default(),
        runtime_evidence: None,
        functions: crate::OptimizationFunctions {
            rows: Vec::new(),
            digest: crate::identity::unit_encoding::function_digest(&[]),
        },
    }
}

fn declarations(unit: &PsiOptimizationUnit, operation: &AbstractOperation) -> Vec<MachineId> {
    unit.call_machine_references(operation)
        .filter_map(|reference| match reference {
            CallMachineReference::Declaration(machine) => Some(machine),
            CallMachineReference::Callable { .. } => None,
        })
        .collect()
}

#[test]
fn direct_callees_are_operation_references_for_both_call_forms() {
    let unit = unit();
    let callee = MachineId::new(2).unwrap();
    let returning = AbstractOperation::Call {
        psi_operation: OperationId::new(1).unwrap(),
        result: ValueId::new(2).unwrap(),
        scalar_type: ScalarType::Boolean,
        callee,
        arguments: vec![ValueId::new(1).unwrap()],
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let diverging = AbstractOperation::InvokeNoNormal {
        psi_edge: EdgeId::new(1).unwrap(),
        callee,
        arguments: vec![ValueId::new(1).unwrap()],
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    for operation in [&returning, &diverging] {
        assert_eq!(declarations(&unit, operation), [callee]);
        assert_eq!(
            unit.operation_machine_references(operation)
                .collect::<Vec<_>>(),
            [callee],
            "{operation:?}"
        );
    }
}
