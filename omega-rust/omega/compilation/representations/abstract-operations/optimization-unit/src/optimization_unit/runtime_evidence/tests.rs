//! The exact context comparison used by every authorized optimizer session.
//! Recomputing content identity does not replace this source-custody judgment.
use super::*;
use crate::{
    attach_verified_module_context, recompute_psi_optimization_unit_identity,
    reconstruct_psi_optimization_unit_seed,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use semantic_vocabulary::{BlockId, ContractId, EdgeId, EvidenceTermId, FuelScheduleIdentity};
use terminal_psi::{
    Block, EvidenceArgumentSubject, EvidenceContractLaneKind, MachineContract,
    RegisterTargetApplicability, SemanticFingerprint, TerminalMachine, TerminalMachineResult,
    Terminator, VocabularyMarker,
};

fn fixture() -> (
    crate::PsiOptimizationUnit,
    TerminalModule,
    TerminalPsiIdentity,
) {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let edge = EdgeId::new(1).unwrap();
    let psi = TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([11; 32]),
    };
    let mut module = TerminalModule::for_entry(machine);
    module.machines.push(TerminalMachine {
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: machine,
        declaration: None,
        attachment: None,
        nominal_cleanup_subject: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge,
                cleanup_actions: Vec::new(),
            },
        )],
        contract: MachineContract::empty(ContractId::new(1).unwrap()),
    });
    module.evidence_contract_lanes.push(EvidenceContractLane {
        machine,
        kind: EvidenceContractLaneKind::Ensures,
        position: 0,
        term: EvidenceTermId::new(1).unwrap(),
        output_field: Some("unused".into()),
        argument_subjects: vec![EvidenceArgumentSubject::Unknown {
            argument_position: 0,
        }],
    });
    let plan = AbstractOperationPlan {
        psi,
        entry: machine,
        structural_types: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        dynamic_dispatch: Default::default(),
        identities: module.identities.clone(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            nominal_cleanup_subject: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![AbstractOperation::ReturnUnit {
                psi_edge: edge,
                cleanup_actions: Vec::new(),
            }],
        }],
    };
    let mut unit =
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap();
    attach_verified_module_context(&mut unit, &module).unwrap();
    (unit, module, psi)
}

#[test]
fn deleting_catalog_and_lanes_then_rehashing_does_not_retain_verified_custody() {
    let (mut unit, module, psi) = fixture();
    assert!(validate_runtime_evidence_context(&unit, psi, &module).is_ok());
    unit.runtime_evidence = None;
    for function in &mut unit.functions {
        function.evidence_contract_lanes.clear();
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert_eq!(
        unit.identity,
        recompute_psi_optimization_unit_identity(&unit)
    );
    assert_eq!(
        validate_runtime_evidence_context(&unit, psi, &module),
        Err("verified optimizer runtime evidence catalog is absent")
    );
}
#[test]
fn replacing_catalog_with_a_same_psi_empty_module_does_not_authorize_deletion() {
    let (mut unit, module, psi) = fixture();
    let mut forged = module.clone();
    forged.evidence_contract_lanes.clear();
    attach_verified_module_context(&mut unit, &forged).unwrap();
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert_eq!(
        unit.identity,
        recompute_psi_optimization_unit_identity(&unit)
    );
    assert_eq!(
        validate_runtime_evidence_context(&unit, psi, &module),
        Err("optimizer runtime evidence catalog differs from admitted Terminal source")
    );
}
#[test]
fn even_empty_verified_evidence_keeps_explicit_context_presence() {
    let (mut unit, mut module, psi) = fixture();
    module.evidence_contract_lanes.clear();
    attach_verified_module_context(&mut unit, &module).unwrap();
    assert!(unit.runtime_evidence.is_some());
    assert!(validate_runtime_evidence_context(&unit, psi, &module).is_ok());
    unit.runtime_evidence = None;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(validate_runtime_evidence_context(&unit, psi, &module).is_err());
}
