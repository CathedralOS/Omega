use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{EvidenceContractLaneKind, EvidenceTermDeclaration};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle, semantic_fingerprint};
use psi_terminal_fixed_fuel::derive_fixed_entry_fuel;
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const FORWARDED_SOURCE: &str = r#"
    trait Evidence<T> {
        machine witness(value: T);
    }

    proposition ready<T>() evidence Evidence<T>;

    data Token { value: u64; }
    data Root {}
    machine Root::forward(first: Token, second: Token)
    requires incoming_first: ready<i32>()
    requires incoming_second: ready<i32>()
    ensures outgoing_first: ready<i32>()
    ensures outgoing_second: ready<i32>()
    {
        outgoing_first = incoming_first;
        outgoing_second = incoming_second;
    }
"#;

const PRODUCED_SOURCE: &str = r#"
    trait Evidence<T> {
        machine witness(value: T);
    }

    proposition ready<T>() evidence Evidence<T>;

    ConcreteEvidence: satisfies Evidence<i32> {
        machine witness(value: i32) {}
    }

    data Token { value: u64; }
    data Root {}
    machine Root::produce(first: Token, second: Token)
    ensures
        outgoing: ready<i32>()
    {
        outgoing = ConcreteEvidence;
    }
"#;

#[test]
fn source_forwarding_preserves_exact_positional_terminal_evidence_identities() {
    let checked = check(FORWARDED_SOURCE);
    assert_eq!(checked.facts.proof.evidence_terms.len(), 4);
    let expected_argument = checked
        .facts
        .proof
        .evidence_terms
        .iter()
        .next()
        .and_then(|(_, term)| term.evidence_interface.as_ref())
        .and_then(|interface| interface.arguments.first())
        .expect("the checked endpoint has one exact instantiated type argument")
        .as_str()
        .to_owned();

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::forward")
        .expect("forwarded witness identity should cross terminal Psi");
    assert_eq!(lowered.semantic_module.evidence_terms.len(), 2);
    let term = &lowered.semantic_module.evidence_terms[0];
    assert_eq!(term.interface.trait_identity, "Evidence");
    assert_eq!(term.interface.arguments, vec![expected_argument]);
    assert_eq!(
        term.proposition,
        lowered.semantic_module.proposition_applications[0].id
    );
    assert_eq!(
        lowered.semantic_module.proposition_applications[0]
            .evidence_interface
            .as_ref(),
        Some(&term.interface)
    );
    let lanes = &lowered.semantic_module.evidence_contract_lanes;
    assert_eq!(lanes.len(), 4);
    assert_eq!(lanes[0].kind, EvidenceContractLaneKind::Requires);
    assert_eq!(lanes[0].position, 0);
    assert_eq!(lanes[1].kind, EvidenceContractLaneKind::Requires);
    assert_eq!(lanes[1].position, 1);
    assert_eq!(lanes[2].kind, EvidenceContractLaneKind::Ensures);
    assert_eq!(lanes[2].position, 0);
    assert_eq!(lanes[3].kind, EvidenceContractLaneKind::Ensures);
    assert_eq!(lanes[3].position, 1);
    assert_eq!(lanes[0].term, lanes[2].term);
    assert_eq!(lanes[1].term, lanes[3].term);
    assert_ne!(lanes[0].term, lanes[1].term);

    let bytes = encode_module(&lowered.semantic_module).expect("terminal evidence should encode");
    assert_eq!(
        decode_module(&bytes).expect("terminal evidence should decode"),
        lowered.semantic_module
    );
    let baseline = semantic_fingerprint(&lowered.semantic_module).expect("terminal identity");

    let mut drifted_term = lowered.semantic_module.clone();
    drifted_term.evidence_terms[0].interface.trait_identity = "OtherEvidence".to_owned();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&drifted_term),
        Err(psi_terminal_verifier::ModuleError::EvidenceTermInterfaceMismatch(_))
    ));

    let mut changed = lowered.semantic_module.clone();
    for term in &mut changed.evidence_terms {
        term.interface.trait_identity = "OtherEvidence".to_owned();
    }
    changed.proposition_applications[0]
        .evidence_interface
        .as_mut()
        .expect("witness application interface")
        .trait_identity = "OtherEvidence".to_owned();
    assert_ne!(
        semantic_fingerprint(&changed).expect("changed terminal identity"),
        baseline,
        "the structured exact interface, not its diagnostic spelling, enters identity"
    );

    let mut unresolved = lowered.semantic_module.clone();
    unresolved.evidence_terms[0]
        .interface
        .trait_identity
        .clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unresolved),
        Err(psi_terminal_verifier::ModuleError::InvalidEvidenceInterface(_))
    ));

    let mut malformed_application = lowered.semantic_module.clone();
    malformed_application.proposition_applications[0]
        .evidence_interface
        .as_mut()
        .expect("witness application interface")
        .trait_identity
        .clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&malformed_application),
        Err(psi_terminal_verifier::ModuleError::InvalidPropositionEvidenceInterface(_))
    ));

    let mut fact_only_application = lowered.semantic_module.clone();
    fact_only_application.proposition_declarations[0].evidence =
        psi_terminal::PropositionEvidence::FactOnly;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&fact_only_application),
        Err(psi_terminal_verifier::ModuleError::InvalidPropositionEvidenceInterface(_))
    ));

    fact_only_application.proposition_applications[0].evidence_interface = None;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&fact_only_application),
        Err(psi_terminal_verifier::ModuleError::FactOnlyEvidenceTerm(_))
    ));

    let mut unknown_proposition = lowered.semantic_module.clone();
    unknown_proposition.evidence_terms[0].proposition =
        psi_core::PropositionId::new(99).expect("test proposition identity");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unknown_proposition),
        Err(psi_terminal_verifier::ModuleError::UnknownEvidenceTermProposition(_))
    ));

    let mut unknown_machine = lowered.semantic_module.clone();
    unknown_machine.evidence_contract_lanes[0].machine =
        psi_core::MachineId::new(99).expect("test machine identity");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unknown_machine),
        Err(psi_terminal_verifier::ModuleError::UnknownEvidenceContractMachine(_))
    ));

    let mut unknown_term = lowered.semantic_module.clone();
    unknown_term.evidence_contract_lanes[0].term =
        psi_core::EvidenceTermId::new(99).expect("test evidence-term identity");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unknown_term),
        Err(psi_terminal_verifier::ModuleError::UnknownEvidenceContractTerm(_))
    ));

    let mut non_dense = lowered.semantic_module.clone();
    non_dense.evidence_contract_lanes[1].position = 2;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&non_dense),
        Err(psi_terminal_verifier::ModuleError::NonDenseEvidenceContractLane { .. })
    ));

    let third = psi_core::EvidenceTermId::new(3).expect("third evidence-term identity");
    let mut unforwarded = lowered.semantic_module.clone();
    unforwarded.evidence_terms.push(EvidenceTermDeclaration {
        id: third,
        proposition: unforwarded.evidence_terms[0].proposition,
        interface: unforwarded.evidence_terms[0].interface.clone(),
    });
    unforwarded.evidence_contract_lanes[2].term = third;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&unforwarded),
        Err(psi_terminal_verifier::ModuleError::UnforwardedEvidenceEnsures { .. })
    ));

    let mut orphan = lowered.semantic_module.clone();
    orphan.evidence_terms.push(EvidenceTermDeclaration {
        id: third,
        proposition: orphan.evidence_terms[0].proposition,
        interface: orphan.evidence_terms[0].interface.clone(),
    });
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&orphan),
        Err(psi_terminal_verifier::ModuleError::OrphanEvidenceTerm(_))
    ));

    let mut reordered = lowered.semantic_module.clone();
    reordered.evidence_contract_lanes.swap(0, 1);
    assert!(matches!(
        encode_module(&reordered),
        Err(psi_terminal_codec::CodecError::NonCanonicalOrder(
            "evidence contract lanes by machine, kind, and position"
        ))
    ));

    let mut remapped = lowered.semantic_module.clone();
    remapped.evidence_contract_lanes.swap(0, 1);
    remapped.evidence_contract_lanes[0].position = 0;
    remapped.evidence_contract_lanes[1].position = 1;
    remapped.evidence_contract_lanes.swap(2, 3);
    remapped.evidence_contract_lanes[2].position = 0;
    remapped.evidence_contract_lanes[3].position = 1;
    assert_ne!(
        semantic_fingerprint(&remapped).expect("remapped lanes remain canonical"),
        baseline,
        "strict lane position-to-term identity enters the fingerprint"
    );

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("forwarding-only erased lanes verify");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("erased evidence lanes admit fixed fuel");
    let mut stripped = lowered.semantic_module.clone();
    stripped.evidence_terms.clear();
    stripped.evidence_contract_lanes.clear();
    let stripped_verified = psi_terminal_verifier::verify_module(
        &stripped,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the executable graph is unchanged without erased rows");
    let stripped_fixed = derive_fixed_entry_fuel(&stripped_verified, stripped.entry)
        .expect("the executable graph keeps fixed fuel");
    assert_eq!(fixed.ceiling_units(), stripped_fixed.ceiling_units());

    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("terminal proof should encode");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let arguments = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0xe710 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("erased evidence lanes require no runtime arguments");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("execute forwarded lanes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

#[test]
fn source_producer_provenance_remains_fail_closed_at_terminal_boundary() {
    let checked = check(PRODUCED_SOURCE);
    let error = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::produce")
        .expect_err("producer provenance has no terminal proof-bundle row yet");
    assert!(
        matches!(
            error,
            psi_checked_trees_to_terminal::LoweringError::Unsupported(
                "terminal evidence producer provenance is not yet serialized separately"
            )
        ),
        "unexpected terminal fence: {error:?}"
    );
}

fn check(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}
