//! Malformed public rosters refuse before any executable snapshot is queried.
use super::*;
use semantic_vocabulary::BlockId;
use terminal_psi::{
    EvidenceInterfaceIdentity, EvidenceTermDeclaration, OutcomeSpecificCallArgumentSubstitution,
    PropositionApplicationIdentity, TerminalIdentityIndex, TerminalIdentityTable,
};

fn check(subjects: Vec<EvidenceArgumentSubject>) -> Result<(), &'static str> {
    let machine = MachineId::new(1).unwrap();
    let proposition = PropositionId::new(1).unwrap();
    let term = EvidenceTermId::new(1).unwrap();
    let interface = EvidenceInterfaceIdentity {
        trait_identity: TerminalIdentityIndex(1),
        arguments: Vec::new(),
        requirements: Vec::new(),
    };
    let identities = TerminalIdentityTable::default();
    let applications = [PropositionApplicationIdentity {
        id: proposition,
        declaration: proposition,
        binder_arguments: Vec::new(),
        arguments: vec!["declared".into()],
        evidence_interface: Some(interface.clone()),
    }];
    let terms = [EvidenceTermDeclaration {
        id: term,
        proposition,
        interface,
    }];
    let lanes = [EvidenceContractLane {
        machine,
        kind: EvidenceContractLaneKind::Requires,
        position: 0,
        term,
        output_field: None,
        argument_subjects: subjects,
    }];
    let bodies = [RuntimeEvidenceBody {
        id: machine,
        entry: BlockId::new(1).unwrap(),
        declaration: None,
        parameters: vec![ValueId::new(1).unwrap()],
        structural_parameters: Vec::new(),
        outcome_specific_ensures: Vec::new(),
        calls: Vec::new(),
        scalar_expressions: Vec::new(),
        payload_contracts: Vec::new(),
    }];
    validate_subject_rosters(&RuntimeEvidenceInputs {
        identities: &identities,
        structural_types: &[],
        structural_domains: &[],
        proposition_applications: &applications,
        evidence_terms: &terms,
        evidence_contract_lanes: &lanes,
        proof_output_calls: &[],
        machines: &bodies,
        graph: &[],
    })
}

#[test]
fn a_missing_subject_is_distinct_from_explicit_unknown() {
    assert!(check(Vec::new()).is_err());
    assert!(
        check(vec![EvidenceArgumentSubject::Unknown {
            argument_position: 0
        }])
        .is_ok()
    );
    assert!(
        check(vec![EvidenceArgumentSubject::Closed {
            argument_position: 1
        }])
        .is_err()
    );
}
#[test]
fn future_results_and_absent_formals_do_not_form_invocation_inputs() {
    assert!(
        check(vec![EvidenceArgumentSubject::Result {
            argument_position: 0
        }])
        .is_err()
    );
    let parameter = |position| {
        EvidenceArgumentSubject::Parameter(OutcomeSpecificCallArgumentSubstitution {
            argument_position: 0,
            parameter_position: position,
            structural_path: None,
        })
    };
    assert!(check(vec![parameter(0)]).is_ok());
    assert!(check(vec![parameter(1)]).is_err());
}

/// Exercise the actual exported Terminal adapter with no Requires or runtime
/// evidence calls. This was the admission path that formerly returned early.
#[test]
fn public_adapter_checks_unused_ensures_subject_rosters() {
    use semantic_vocabulary::{ContractId, EdgeId};
    use terminal_psi::{
        Block, MachineContract, RegisterTargetApplicability, TerminalMachine,
        TerminalMachineResult, Terminator,
    };
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let proposition = PropositionId::new(1).unwrap();
    let term = EvidenceTermId::new(1).unwrap();
    let interface = EvidenceInterfaceIdentity {
        trait_identity: TerminalIdentityIndex(1),
        arguments: Vec::new(),
        requirements: Vec::new(),
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
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract: MachineContract::empty(ContractId::new(1).unwrap()),
    });
    module
        .proposition_applications
        .push(PropositionApplicationIdentity {
            id: proposition,
            declaration: proposition,
            binder_arguments: Vec::new(),
            arguments: vec!["closed".into()],
            evidence_interface: Some(interface.clone()),
        });
    module.evidence_terms.push(EvidenceTermDeclaration {
        id: term,
        proposition,
        interface,
    });
    module.evidence_contract_lanes.push(EvidenceContractLane {
        machine,
        kind: EvidenceContractLaneKind::Ensures,
        position: 0,
        term,
        output_field: Some("unused".into()),
        argument_subjects: vec![EvidenceArgumentSubject::Closed {
            argument_position: 0,
        }],
    });
    assert!(crate::validate_runtime_evidence_inputs(&module).is_ok());
    module.evidence_contract_lanes[0].argument_subjects.clear();
    assert_eq!(
        crate::validate_runtime_evidence_inputs(&module),
        Err("evidence declaration has no complete exact typed subject roster")
    );
    module.evidence_contract_lanes[0]
        .argument_subjects
        .push(EvidenceArgumentSubject::Closed {
            argument_position: 1,
        });
    assert_eq!(
        crate::validate_runtime_evidence_inputs(&module),
        Err("evidence declaration has no complete exact typed subject roster")
    );
}
