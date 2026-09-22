use super::{
    ARGUMENTED_PROOF_OUTPUT_SOURCE, DUPLICATE_ARGUMENTED_PROOF_OUTPUT_SOURCE, FORWARDED_SOURCE,
    PRODUCED_SOURCE, PROJECTED_SOURCE, PROOF_OUTPUT_SOURCE,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{
    decode_module, decode_proof_bundle, encode_module, encode_proof_section,
    proof_bundle_fingerprint, render_verified_proof_synopsis, semantic_fingerprint,
};
use terminal_fixed_fuel::derive_fixed_entry_fuel;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};
use terminal_psi::{EvidenceContractLaneKind, EvidenceTermDeclaration};

#[test]
fn source_projection_uses_canonical_term_and_exact_requirement_identity() {
    let checked = crate::front_end::checked_program(PROJECTED_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::project"),
    )
    .expect("carrierless projection should cross terminal Psi");
    let projections = lowered
        .semantic_module
        .proposition_applications
        .iter()
        .flat_map(|application| &application.binder_arguments)
        .filter_map(|argument| argument.evidence_projection.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(projections.len(), 2, "the repeated projection deduplicates");
    assert_eq!(
        projections[0].requirement_identity,
        projections[1].requirement_identity
    );
    assert_eq!(projections[0].declaring_trait_identity, "Parent");
    assert_eq!(projections[0].declaring_trait_arguments.len(), 1);
    assert_ne!(
        projections[0].term, projections[1].term,
        "separate evidence introductions retain distinct opaque projections"
    );
    assert!(lowered.semantic_module.evidence_terms.iter().all(|term| {
        term.interface.requirements.iter().any(|requirement| {
            requirement.requirement_identity == projections[0].requirement_identity
        })
    }));

    let bytes = encode_module(&lowered.semantic_module).expect("projection module encodes");
    assert_eq!(
        decode_module(&bytes).expect("projection module decodes"),
        lowered.semantic_module
    );

    let mut wrong_term = lowered.semantic_module.clone();
    let projection = wrong_term
        .proposition_applications
        .iter_mut()
        .flat_map(|application| &mut application.binder_arguments)
        .find_map(|argument| argument.evidence_projection.as_mut())
        .expect("projection");
    projection.term = semantic_vocabulary::EvidenceTermId::new(99).expect("test evidence term");
    assert!(matches!(
        terminal_verifier::validate_module_representation(&wrong_term),
        Err(terminal_verifier::ModuleError::UnknownEvidenceProjectionTerm { .. })
    ));

    let mut wrong_requirement = lowered.semantic_module.clone();
    wrong_requirement
        .proposition_applications
        .iter_mut()
        .flat_map(|application| &mut application.binder_arguments)
        .find_map(|argument| argument.evidence_projection.as_mut())
        .expect("projection")
        .requirement_identity = "forged requirement".to_owned();
    assert!(matches!(
        terminal_verifier::validate_module_representation(&wrong_requirement),
        Err(terminal_verifier::ModuleError::EvidenceProjectionRequirementMismatch { .. })
    ));
}

#[test]
fn forwarded_projection_uses_the_shared_terminal_term_identity() {
    let checked = crate::front_end::checked_program(PROJECTED_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::forward"),
    )
    .expect("forwarded carrierless projection should cross terminal Psi");
    let projection = lowered
        .semantic_module
        .proposition_applications
        .iter()
        .flat_map(|application| &application.binder_arguments)
        .find_map(|argument| argument.evidence_projection.as_ref())
        .expect("projection");
    let requires = lowered
        .semantic_module
        .evidence_contract_lanes
        .iter()
        .find(|lane| lane.kind == EvidenceContractLaneKind::Requires)
        .expect("requires lane");
    let ensures = lowered
        .semantic_module
        .evidence_contract_lanes
        .iter()
        .find(|lane| lane.kind == EvidenceContractLaneKind::Ensures)
        .expect("ensures lane");
    assert_eq!(requires.term, ensures.term);
    assert_eq!(projection.term, requires.term);
}

#[test]
fn source_forwarding_preserves_exact_positional_terminal_evidence_identities() {
    let checked = crate::front_end::checked_program(FORWARDED_SOURCE);
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

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::forward"),
    )
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
    assert_eq!(lanes[0].output_field, None);
    assert_eq!(lanes[1].kind, EvidenceContractLaneKind::Requires);
    assert_eq!(lanes[1].position, 1);
    assert_eq!(lanes[1].output_field, None);
    assert_eq!(lanes[2].kind, EvidenceContractLaneKind::Ensures);
    assert_eq!(lanes[2].position, 0);
    assert_eq!(lanes[2].output_field.as_deref(), Some("outgoing_first"));
    assert_eq!(lanes[3].kind, EvidenceContractLaneKind::Ensures);
    assert_eq!(lanes[3].position, 1);
    assert_eq!(lanes[3].output_field.as_deref(), Some("outgoing_second"));
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
        terminal_verifier::validate_module_representation(&drifted_term),
        Err(terminal_verifier::ModuleError::EvidenceTermInterfaceMismatch(_))
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
        terminal_verifier::validate_module_representation(&unresolved),
        Err(terminal_verifier::ModuleError::InvalidEvidenceInterface(_))
    ));

    let mut malformed_application = lowered.semantic_module.clone();
    malformed_application.proposition_applications[0]
        .evidence_interface
        .as_mut()
        .expect("witness application interface")
        .trait_identity
        .clear();
    assert!(matches!(
        terminal_verifier::validate_module_representation(&malformed_application),
        Err(terminal_verifier::ModuleError::InvalidPropositionEvidenceInterface(_))
    ));

    let mut fact_only_application = lowered.semantic_module.clone();
    fact_only_application.proposition_declarations[0].evidence =
        terminal_psi::PropositionEvidence::FactOnly;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&fact_only_application),
        Err(terminal_verifier::ModuleError::InvalidPropositionEvidenceInterface(_))
    ));

    fact_only_application.proposition_applications[0].evidence_interface = None;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&fact_only_application),
        Err(terminal_verifier::ModuleError::FactOnlyEvidenceTerm(_))
    ));

    let mut unknown_proposition = lowered.semantic_module.clone();
    unknown_proposition.evidence_terms[0].proposition =
        semantic_vocabulary::PropositionId::new(99).expect("test proposition identity");
    assert!(matches!(
        terminal_verifier::validate_module_representation(&unknown_proposition),
        Err(terminal_verifier::ModuleError::UnknownEvidenceTermProposition(_))
    ));

    let mut unknown_machine = lowered.semantic_module.clone();
    unknown_machine.evidence_contract_lanes[0].machine =
        semantic_vocabulary::MachineId::new(99).expect("test machine identity");
    assert!(matches!(
        terminal_verifier::validate_module_representation(&unknown_machine),
        Err(terminal_verifier::ModuleError::UnknownEvidenceContractMachine(_))
    ));

    let mut unknown_term = lowered.semantic_module.clone();
    unknown_term.evidence_contract_lanes[0].term =
        semantic_vocabulary::EvidenceTermId::new(99).expect("test evidence-term identity");
    assert!(matches!(
        terminal_verifier::validate_module_representation(&unknown_term),
        Err(terminal_verifier::ModuleError::UnknownEvidenceContractTerm(
            _
        ))
    ));

    let mut non_dense = lowered.semantic_module.clone();
    non_dense.evidence_contract_lanes[1].position = 2;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&non_dense),
        Err(terminal_verifier::ModuleError::NonDenseEvidenceContractLane { .. })
    ));

    let third = semantic_vocabulary::EvidenceTermId::new(3).expect("third evidence-term identity");
    let mut unforwarded = lowered.semantic_module.clone();
    unforwarded.evidence_terms.push(EvidenceTermDeclaration {
        id: third,
        proposition: unforwarded.evidence_terms[0].proposition,
        interface: unforwarded.evidence_terms[0].interface.clone(),
    });
    unforwarded.evidence_contract_lanes[2].term = third;
    terminal_verifier::validate_module_representation(&unforwarded)
        .expect("proof provenance, not semantic validation, owns fresh evidence");
    assert!(matches!(
        terminal_verifier::verify_module(
            &unforwarded,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidenceProducer(term))
            if term == third
    ));

    let mut orphan = lowered.semantic_module.clone();
    orphan.evidence_terms.push(EvidenceTermDeclaration {
        id: third,
        proposition: orphan.evidence_terms[0].proposition,
        interface: orphan.evidence_terms[0].interface.clone(),
    });
    assert!(matches!(
        terminal_verifier::validate_module_representation(&orphan),
        Err(terminal_verifier::ModuleError::OrphanEvidenceTerm(_))
    ));

    let mut reordered = lowered.semantic_module.clone();
    reordered.evidence_contract_lanes.swap(0, 1);
    assert!(matches!(
        encode_module(&reordered),
        Err(terminal_codec::CodecError::NonCanonicalOrder(
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

    let mut renamed = lowered.semantic_module.clone();
    renamed.evidence_contract_lanes[2].output_field = Some("renamed".to_owned());
    assert_ne!(
        semantic_fingerprint(&renamed).expect("renamed output remains valid"),
        baseline,
        "the public proof-output field enters semantic identity"
    );

    let mut missing_field = lowered.semantic_module.clone();
    missing_field.evidence_contract_lanes[2].output_field = None;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&missing_field),
        Err(terminal_verifier::ModuleError::MissingEvidenceOutputField { .. })
    ));

    let mut input_field = lowered.semantic_module.clone();
    input_field.evidence_contract_lanes[0].output_field = Some("incoming_first".to_owned());
    assert!(matches!(
        terminal_verifier::validate_module_representation(&input_field),
        Err(terminal_verifier::ModuleError::EvidenceRequiresHasOutputField { .. })
    ));

    let mut reserved = lowered.semantic_module.clone();
    reserved.evidence_contract_lanes[2].output_field = Some("value".to_owned());
    assert!(matches!(
        terminal_verifier::validate_module_representation(&reserved),
        Err(terminal_verifier::ModuleError::ReservedEvidenceOutputField(
            _
        ))
    ));

    let mut duplicate = lowered.semantic_module.clone();
    let field = duplicate.evidence_contract_lanes[2].output_field.clone();
    duplicate.evidence_contract_lanes[3].output_field = field;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&duplicate),
        Err(terminal_verifier::ModuleError::DuplicateEvidenceOutputField(_))
    ));

    let verified = terminal_verifier::verify_module(
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
    let stripped_verified = terminal_verifier::verify_module(
        &stripped,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the executable graph is unchanged without erased rows");
    let stripped_fixed = derive_fixed_entry_fuel(&stripped_verified, stripped.entry)
        .expect("the executable graph keeps fixed fuel");
    assert_eq!(fixed.ceiling_units(), stripped_fixed.ceiling_units());

    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("terminal proof should encode");
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
    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
    )
    .expect("erased evidence lanes require no runtime arguments");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("execute forwarded lanes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

#[test]
fn source_producer_provenance_is_separate_canonical_verified_proof_data() {
    let checked = crate::front_end::checked_program(PRODUCED_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::produce"),
    )
    .expect("selected producer provenance should cross terminal Psi");
    assert_eq!(lowered.semantic_module.evidence_terms.len(), 1);
    assert_eq!(lowered.semantic_module.evidence_contract_lanes.len(), 1);
    assert_eq!(
        lowered.semantic_module.evidence_contract_lanes[0].kind,
        EvidenceContractLaneKind::Ensures
    );
    assert_eq!(
        lowered.semantic_module.evidence_contract_lanes[0]
            .output_field
            .as_deref(),
        Some("outgoing")
    );
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);
    let producer = &lowered.proof_bundle.evidence_producers[0];
    assert_eq!(producer.id.get(), 1);
    assert_eq!(
        producer.term,
        lowered.semantic_module.evidence_contract_lanes[0].term
    );
    assert_eq!(producer.conformance_identity, "ConcreteEvidence");
    assert_eq!(producer.evidence_trait_identity, "Evidence");
    assert_eq!(producer.rows.len(), 1);
    assert!(!producer.rows[0].requirement_identity.is_empty());
    assert_eq!(
        producer.rows[0].declaring_trait_arguments,
        lowered.semantic_module.evidence_terms[0]
            .interface
            .requirements[0]
            .declaring_trait_arguments
    );

    let semantic = semantic_fingerprint(&lowered.semantic_module).expect("terminal identity");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("producer proof encodes");
    assert_eq!(
        decode_proof_bundle(&proof),
        Ok(lowered.proof_bundle.clone())
    );
    let proof_fingerprint =
        proof_bundle_fingerprint(&lowered.proof_bundle).expect("proof identity");
    let mut changed_proof = lowered.proof_bundle.clone();
    changed_proof.evidence_producers[0].conformance_identity = "OtherEvidence".to_owned();
    assert_ne!(
        proof_bundle_fingerprint(&changed_proof).expect("changed proof remains canonical"),
        proof_fingerprint
    );
    assert_eq!(
        semantic_fingerprint(&lowered.semantic_module).expect("semantic identity is independent"),
        semantic
    );

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("exact selected producer provenance verifies");

    let mut missing = lowered.proof_bundle.clone();
    missing.evidence_producers.clear();
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing,
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::MissingEvidenceProducer(_))
    ));

    let mut wrong_trait = lowered.proof_bundle.clone();
    wrong_trait.evidence_producers[0].evidence_trait_identity = "OtherEvidence".to_owned();
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &wrong_trait,
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::EvidenceProducerInterfaceMismatch(_))
    ));

    let mut non_dense = lowered.proof_bundle.clone();
    non_dense.evidence_producers[0].id =
        semantic_vocabulary::EvidenceIdentity::new(2).expect("test proof identity");
    assert_eq!(
        encode_proof_section(&lowered.semantic_module, &non_dense),
        Err(terminal_codec::ProofCodecError::NonCanonicalEvidenceProducerOrder)
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &non_dense,
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::NonDenseEvidenceProducer { .. })
    ));

    let mut malformed_row = lowered.proof_bundle.clone();
    malformed_row.evidence_producers[0].rows[0]
        .requirement_identity
        .clear();
    assert_eq!(
        encode_proof_section(&lowered.semantic_module, &malformed_row),
        Err(terminal_codec::ProofCodecError::InvalidEvidenceProducer)
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &malformed_row,
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::InvalidEvidenceProducer(_))
    ));

    let mut duplicate_row = lowered.proof_bundle.clone();
    let row = duplicate_row.evidence_producers[0].rows[0].clone();
    duplicate_row.evidence_producers[0].rows.push(row);
    assert_eq!(
        encode_proof_section(&lowered.semantic_module, &duplicate_row),
        Err(terminal_codec::ProofCodecError::NonCanonicalEvidenceProducerRows)
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &duplicate_row,
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::NonCanonicalEvidenceProducerRows(_))
    ));

    let mut wrong_complete_map = lowered.proof_bundle.clone();
    wrong_complete_map.evidence_producers[0].rows[0].requirement_identity =
        "different complete row".to_owned();
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &wrong_complete_map,
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::EvidenceProducerInterfaceMismatch(_))
    ));

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("producer proof verifies for fuel derivation");
    let synopsis = render_verified_proof_synopsis(&verified).expect("producer audit synopsis");
    assert!(
        synopsis.contains("evidence-producer 1 term 1 conformance ConcreteEvidence trait Evidence")
    );
    assert!(synopsis.contains("  row Evidence "));
    let fuel = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("producer provenance does not prevent fixed fuel");
    let mut stripped_module = lowered.semantic_module.clone();
    stripped_module.evidence_terms.clear();
    stripped_module.evidence_contract_lanes.clear();
    let stripped_bundle = terminal_verifier::ProofBundle::default();
    let stripped = terminal_verifier::verify_module(
        &stripped_module,
        &stripped_bundle,
        &AdmissionProfile::default(),
    )
    .expect("erased evidence does not change execution");
    assert_eq!(
        derive_fixed_entry_fuel(&stripped, stripped_module.entry)
            .expect("stripped machine has fixed fuel")
            .ceiling_units(),
        fuel.ceiling_units()
    );

    let bytes = encode_module(&lowered.semantic_module).expect("producer module encodes");
    let machine = &lowered.semantic_module.machines[0];
    let arguments = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0xe720 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
    )
    .expect("producer evidence remains erased at runtime");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("execute producer"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

#[test]
fn argumented_proof_output_retains_substitution_and_erased_input_identity() {
    let checked = crate::front_end::checked_program(ARGUMENTED_PROOF_OUTPUT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("argumented proof-output call should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal proof-output invocation expected")
    };
    let [argument] = invocation.evidence_arguments.as_slice() else {
        panic!("one terminal erased input expected")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("one terminal proof output expected")
    };
    let term = |id| {
        lowered
            .semantic_module
            .evidence_terms
            .iter()
            .find(|term| term.id == id)
            .expect("proof-output term identity")
    };
    assert_eq!(argument.input_position, 0);
    assert_eq!(
        term(argument.source).proposition,
        argument.instantiated_proposition
    );
    assert_ne!(
        argument.callee_proposition, argument.instantiated_proposition,
        "formal and call-substituted propositions remain distinct"
    );
    assert_eq!(
        term(output.output.expect("captured output")).proposition,
        output.instantiated_proposition
    );
    assert_eq!(output.callee_output, None);
    assert_ne!(output.callee_proposition, output.instantiated_proposition);

    let bytes = encode_module(&lowered.semantic_module).expect("argumented proof output encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("argument substitution and erased evidence input verify together");

    let mut wrong_input = lowered.semantic_module.clone();
    wrong_input.proof_output_calls[0].evidence_arguments[0].instantiated_proposition =
        argument.callee_proposition;
    assert!(terminal_verifier::validate_module_representation(&wrong_input).is_err());

    let mut wrong_output = lowered.semantic_module.clone();
    wrong_output.proof_output_calls[0].outputs[0].instantiated_proposition =
        output.callee_proposition;
    assert!(terminal_verifier::validate_module_representation(&wrong_output).is_err());
}

#[test]
fn generic_proof_output_target_identity_binds_the_closed_conformance_application() {
    fn source(selection: &str) -> String {
        format!(
            r#"
                trait Evidence {{}}
                trait Marker {{}}
                proposition ready() evidence Evidence;
                data Card {{}}
                data Root {{ card: Card; }}
                {selection}: Card satisfies Marker {{}}

                machine produce<Element, Selection: Element satisfies Marker>(value: &Element)
                requires incoming: ready()
                ensures copied: ready()
                {{ copied = incoming; }}

                machine Root::relay(&self)
                requires source: ready()
                ensures relayed: ready()
                {{
                    let (; copied: local) = produce<Card, {selection}>(&self.card; source);
                    relayed = local;
                }}
            "#
        )
    }
    fn selected_specialization(
        checked: &checked_trees::CheckedTrees,
    ) -> &typed_trees::typed_trees::MachineSpecialization {
        let target = checked
            .facts
            .proof
            .proof_output_calls
            .iter()
            .next()
            .map(|(_, invocation)| invocation.target_machine_symbol)
            .expect("one checked generic proof-output call");
        checked
            .machine_specializations
            .iter()
            .find(|specialization| specialization.instance == target)
            .expect("the target should retain its checked specialization")
    }

    fn commitment_hex(
        commitment: typed_trees::typed_trees::MachineSpecializationCommitment,
    ) -> String {
        use std::fmt::Write;

        let mut rendered = String::with_capacity(64);
        for byte in commitment.as_bytes() {
            write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
        }
        rendered
    }

    let first = crate::front_end::checked_program(&source("FirstMarker"));
    let second = crate::front_end::checked_program(&source("SecondMarker"));
    let first_specialization = selected_specialization(&first);
    let second_specialization = selected_specialization(&second);
    assert_ne!(
        first_specialization.report_fingerprint,
        second_specialization.report_fingerprint,
    );
    assert_ne!(
        first_specialization.commitment,
        second_specialization.commitment,
    );
    let first_commitment = commitment_hex(first_specialization.commitment);
    let second_commitment = commitment_hex(second_specialization.commitment);
    let first_lowered = checked_trees_to_lowered_psi::lower_machine(
        &first,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("first closed generic proof output should lower");
    let first_replay = checked_trees_to_lowered_psi::lower_machine(
        &first,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("the same exact specialization should lower deterministically");
    let second_lowered = checked_trees_to_lowered_psi::lower_machine(
        &second,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("second closed generic proof output should lower");
    let [first_call] = first_lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one first proof-output call expected")
    };
    let [first_replay_call] = first_replay.semantic_module.proof_output_calls.as_slice() else {
        panic!("one replayed proof-output call expected")
    };
    let [second_call] = second_lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one second proof-output call expected")
    };
    assert!(
        first_call
            .target_machine_identity
            .starts_with("specialized-machine|")
    );
    assert!(
        first_call
            .target_machine_identity
            .ends_with(&format!("application={first_commitment}"))
    );
    assert!(
        second_call
            .target_machine_identity
            .ends_with(&format!("application={second_commitment}"))
    );
    assert_eq!(
        first_call.target_machine_identity, first_replay_call.target_machine_identity,
        "the same exact specialization has one deterministic producer identity",
    );
    assert_ne!(
        first_call.target_machine_identity, second_call.target_machine_identity,
        "different closed conformance selections are different proof-output targets"
    );
    let baseline =
        semantic_fingerprint(&first_lowered.semantic_module).expect("generic target identity");
    let mut tampered = first_lowered.semantic_module.clone();
    let identity = &mut tampered.proof_output_calls[0].target_machine_identity;
    let last = identity.len() - 1;
    let replacement = if identity.ends_with('0') { "1" } else { "0" };
    identity.replace_range(last.., replacement);
    assert_ne!(
        semantic_fingerprint(&tampered).expect("tampered generic target remains canonical"),
        baseline,
        "the exact closed application enters terminal semantic identity"
    );

    let mut changed_exact_specialization = first.clone();
    let specialization = selected_specialization(&changed_exact_specialization);
    let target = specialization.instance;
    let retained_report = specialization.report_fingerprint;
    let specialization = changed_exact_specialization
        .typed
        .machine_specializations
        .iter_mut()
        .find(|specialization| specialization.instance == target)
        .expect("mutated specialization remains present");
    specialization.type_argument_identities[0].push_str("|substituted");
    assert_eq!(specialization.report_fingerprint, retained_report);
    assert_eq!(
        checked_trees_to_lowered_psi::lower_machine(
            &changed_exact_specialization,
            TerminalMachineSelection::Name("Root::relay"),
        ),
        Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
            "evidence machine specialization commitment does not replay",
        )),
        "an exact specialization mutation cannot hide behind the same compact report coordinate",
    );

    let mut substituted_commitment = first.clone();
    let specialization = selected_specialization(&substituted_commitment);
    let target = specialization.instance;
    let retained_report = specialization.report_fingerprint;
    let specialization = substituted_commitment
        .typed
        .machine_specializations
        .iter_mut()
        .find(|specialization| specialization.instance == target)
        .expect("substituted specialization remains present");
    specialization.commitment = selected_specialization(&second).commitment;
    assert_eq!(specialization.report_fingerprint, retained_report);
    assert_eq!(
        checked_trees_to_lowered_psi::lower_machine(
            &substituted_commitment,
            TerminalMachineSelection::Name("Root::relay")
        ),
        Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
            "evidence machine specialization commitment does not replay",
        )),
        "a stored commitment from another exact specialization must not be admitted",
    );
}

#[test]
fn forwarded_proof_output_retains_the_exact_duplicate_input_lane() {
    let checked = crate::front_end::checked_program(DUPLICATE_ARGUMENTED_PROOF_OUTPUT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("duplicate proof inputs should retain exact lane identity");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal proof-output invocation expected")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("one terminal proof output expected")
    };
    assert_eq!(invocation.evidence_arguments.len(), 2);
    assert_eq!(output.forwarded_input_position, Some(1));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("forwarding the second duplicate witness lane should verify");
}

#[test]
fn proof_output_is_canonical_verified_and_runtime_erased() {
    let checked = crate::front_end::checked_program(PROOF_OUTPUT_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::relay"),
    )
    .expect("proof-output call should cross terminal Psi");
    let [invocation] = lowered.semantic_module.proof_output_calls.as_slice() else {
        panic!("one terminal proof-output invocation expected")
    };
    assert_eq!(invocation.ordinal, 0);
    let [output] = invocation.outputs.as_slice() else {
        panic!("one terminal proof-output output expected")
    };
    assert_eq!(output.output_position, 0);
    assert_eq!(output.output_field, "outgoing");
    assert_ne!(
        output.callee_output.expect("producer-backed callee output"),
        output.output.expect("bound output")
    );
    assert_eq!(lowered.proof_bundle.evidence_producers.len(), 1);

    let bytes = encode_module(&lowered.semantic_module).expect("proof-output module encodes");
    assert_eq!(decode_module(&bytes), Ok(lowered.semantic_module.clone()));
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof-output proof encodes");
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("exact proof-output call and callee producer verify");
    derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("proof-only invocation adds no runtime fuel obligation");

    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("proof-only proof output requires no runtime argument");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("execute erased proof output"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );

    let mut forged = lowered.semantic_module.clone();
    forged.proof_output_calls[0].outputs[0].output =
        forged.proof_output_calls[0].outputs[0].callee_output;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&forged),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));

    let baseline = semantic_fingerprint(&lowered.semantic_module).expect("proof-output identity");
    let mut renamed_target = lowered.semantic_module.clone();
    renamed_target.proof_output_calls[0].target_machine_identity =
        "different canonical producer".to_owned();
    assert_ne!(
        semantic_fingerprint(&renamed_target).expect("renamed target is distinct semantics"),
        baseline
    );
    let mut renamed_field = lowered.semantic_module.clone();
    renamed_field.proof_output_calls[0].outputs[0].output_field = "renamed".to_owned();
    assert_ne!(
        semantic_fingerprint(&renamed_field).expect("renamed field is distinct semantics"),
        baseline
    );
    let mut non_dense = lowered.semantic_module.clone();
    non_dense.proof_output_calls[0].ordinal = 1;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&non_dense),
        Err(terminal_verifier::ModuleError::NonCanonicalProofOutputCall { .. })
    ));
    let mut empty_target = lowered.semantic_module.clone();
    empty_target.proof_output_calls[0]
        .target_machine_identity
        .clear();
    assert!(matches!(
        terminal_verifier::validate_module_representation(&empty_target),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
    let mut reserved_field = lowered.semantic_module.clone();
    reserved_field.proof_output_calls[0].outputs[0].output_field = "value".to_owned();
    assert!(matches!(
        terminal_verifier::validate_module_representation(&reserved_field),
        Err(terminal_verifier::ModuleError::InvalidProofOutputCall { .. })
    ));
}
