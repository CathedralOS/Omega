//! Check the production optimized projection, including its actual entry roots.

use super::*;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractParameter, AbstractResult, AbstractStructuralCallResult,
};
use semantic_vocabulary::{
    BlockId, CallableContractId, ContractId, EdgeId, MachineDescriptorId, MachineId, OperationId,
    ScalarType, ValueId, WorkScheduleIdentity,
};
use std::collections::BTreeSet;
use terminal_psi::{
    BoundaryMachineResult, MachineContract, SemanticFingerprint, TerminalCallableCatalog,
    TerminalCallableContract, TerminalCallableEnvelope, TerminalCallableTermination,
    TerminalIdentityIndex, TerminalMachineDescriptor, TerminalMachineDescriptorOrigin,
    TerminalMachineResult, TerminalPsiIdentity, VocabularyMarker,
};
use terminal_semantics::callable_values::{ReachedCallableTarget, reachable_callable_invocations};

fn machine(id: u64) -> MachineId {
    MachineId::new(id).unwrap()
}
fn operation(id: u64) -> OperationId {
    OperationId::new(id).unwrap()
}
fn value(id: u64) -> ValueId {
    ValueId::new(id).unwrap()
}
fn contract() -> CallableContractId {
    CallableContractId::new(1).unwrap()
}
fn descriptor(id: u64) -> MachineDescriptorId {
    MachineDescriptorId::new(id).unwrap()
}
fn function(
    id: u64,
    parameters: Vec<AbstractParameter>,
    mut operations: Vec<AbstractOperation>,
) -> AbstractFunction {
    let entry = BlockId::new(1).unwrap();
    operations.push(AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(1).unwrap(),
        cleanup_actions: Vec::new(),
    });
    AbstractFunction {
        machine: machine(id),
        attachment: None,
        nominal_cleanup_subject: None,
        entry,
        parameters,
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            block: entry,
            structural_parameters: Vec::new(),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations,
    }
}
fn call(id: u64, callee: AbstractCallTarget, arguments: Vec<ValueId>) -> AbstractOperation {
    AbstractOperation::CallStructural {
        psi_operation: operation(id),
        callee,
        arguments,
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        result: AbstractStructuralCallResult::Unit,
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    }
}
fn catalog() -> TerminalCallableCatalog {
    TerminalCallableCatalog {
        contracts: vec![TerminalCallableContract {
            id: contract(),
            identity: TerminalIdentityIndex(1),
            scalar_parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Unit,
            proof_result: TerminalMachineResult::Unit,
            entry_claim_paths: Vec::new(),
            result_claim_paths: Vec::new(),
            result_reference_regions: Vec::new(),
            proof: MachineContract::empty(ContractId::new(1).unwrap()),
            evidence_contract: Vec::new(),
            envelope: TerminalCallableEnvelope {
                service_reach: Vec::new(),
                invocations: Vec::new(),
                suspends: false,
                blocks: false,
                termination: TerminalCallableTermination::NotGuaranteed,
            },
        }],
        descriptors: [(1, 3), (2, 4), (3, 5)]
            .into_iter()
            .map(|(id, target)| TerminalMachineDescriptor {
                id: descriptor(id),
                contract: contract(),
                machine: machine(target),
                origin: TerminalMachineDescriptorOrigin {
                    application: TerminalIdentityIndex(id as u32 + 1),
                    conformance: None,
                    requirement: None,
                },
                refinement_obligations: Vec::new(),
                refinement_evidence: Vec::new(),
            })
            .collect(),
        machine_envelopes: Vec::new(),
        boundary_envelopes: Vec::new(),
    }
}

#[test]
fn optimized_entry_calls_resolve_formal_actuals_without_eligible_code_rosters() {
    let mut operations = Vec::new();
    for id in 1..=3 {
        operations.push(AbstractOperation::SelectMachine {
            psi_operation: operation(id),
            result: AbstractResult {
                value: value(id),
                scalar_type: ScalarType::Callable(contract()),
            },
            descriptor: descriptor(id),
        });
    }
    operations.push(call(
        4,
        AbstractCallTarget::Machine(machine(2)),
        vec![value(1)],
    ));
    operations.push(call(
        5,
        AbstractCallTarget::Machine(machine(2)),
        vec![value(2)],
    ));
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([31; 32]),
        },
        entry: machine(1),
        structural_types: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        callables: catalog(),
        dynamic_dispatch: Default::default(),
        identities: Default::default(),
        functions: vec![
            function(1, Vec::new(), operations),
            function(
                2,
                vec![AbstractParameter {
                    value: value(1),
                    scalar_type: ScalarType::Callable(contract()),
                }],
                vec![call(
                    1,
                    AbstractCallTarget::Callable {
                        callee: value(1),
                        contract: contract(),
                    },
                    Vec::new(),
                )],
            ),
            function(3, Vec::new(), Vec::new()),
            function(4, Vec::new(), Vec::new()),
            function(5, Vec::new(), Vec::new()),
        ],
    };
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        WorkScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let projected = optimized_callable_values(&unit).unwrap();
    let reached = reachable_callable_invocations(&projected, [unit.entry]).unwrap();
    let invocation = reached
        .operation(machine(2), operation(1))
        .unwrap()
        .unwrap();
    let ReachedCallableTarget::Callable(target) = &invocation.target else {
        panic!("optimized invocation lost its evaluated callable operand");
    };
    assert_eq!(
        target.descriptors,
        BTreeSet::from([descriptor(1), descriptor(2)])
    );
    assert!(invocation.targets_are_resolved());
    assert_eq!(
        invocation.selected_machines,
        BTreeSet::from([machine(3), machine(4)])
    );
    assert_eq!(
        reached.reached_machines,
        BTreeSet::from([machine(1), machine(2), machine(3), machine(4)])
    );
    assert!(
        reached
            .operation(machine(1), operation(3))
            .unwrap()
            .is_none()
    );
    assert!(!reached.reached_machines.contains(&machine(5)));
}
