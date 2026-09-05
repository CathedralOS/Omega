use proof_admission::{AcceptedFactRoute, AcceptedProofRule, AdmissionProfile, EvidenceRoute};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_psi::OperationKind;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(token: Token, value: u8) -> bool
    requires value <= 63u8
    {
        (value << 2u8) < 255u8
    }
"#;

#[test]
fn bounded_exact_left_shift_uses_only_its_canonical_certificate() {
    let tokens = Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize exact left shift");
    let syntax = parse_syntax_trees(&tokens).expect("parse exact left shift");
    let resolved = lower_syntax_trees(&syntax).expect("resolve exact left shift");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type exact left shift");
    let checked = lower_typed_trees(typed).expect("check exact left shift");
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::enter")
        .expect("bounded exact left shift lowers with a producer certificate");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftLeft { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("one exact-left-shift obligation");
    let reconstructed =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("reconstruct exact-left-shift obligations")
            .into_iter()
            .find(|site| site.obligation.id == obligation)
            .expect("reconstruct the exact-left-shift site");
    assert!(
        reconstructed.canonical_certificate,
        "exact-left-shift representability must not regain sufficient-reducer authority",
    );

    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == obligation)
        .expect("exact left shift retains evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
        panic!("exact left shift uses the canonical certificate route")
    };
    assert_eq!(
        certificate.proof.conclusion, reconstructed.obligation.proposition,
        "the producer certificate binds the verifier-reconstructed question",
    );

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the verifier accepts the bounded exact-left-shift certificate");
    let accepted = verified
        .accepted_facts()
        .iter()
        .find(|fact| fact.obligation == obligation)
        .expect("verified exact left shift publishes accepted evidence");
    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = &accepted.route else {
        panic!("verified exact left shift reports certificate-derived authority")
    };
    assert!(
        acceptance
            .rules
            .contains(&AcceptedProofRule::IntegerAffineBound),
        "accepted exact-left-shift evidence reports the checked endpoint-transform rule",
    );

    let semantic_bytes = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");

    let mut mutated_proof = decode_proof_bundle(&proof_bytes).expect("decode proof mutation");
    let mutated_evidence = mutated_proof
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == obligation)
        .expect("exact left shift retains mutable evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &mut mutated_evidence.route else {
        panic!("exact left shift retains a recursive certificate")
    };
    certificate.proof.conclusion = Proposition::Truth;
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantic_bytes).expect("decode unchanged semantics"),
            &mutated_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation: rejected,
            ..
        }) if rejected == obligation
    ));

    let mut missing_proof = decode_proof_bundle(&proof_bytes).expect("decode missing proof");
    missing_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        terminal_verifier::verify_module(
            &decode_module(&semantic_bytes).expect("decode unchanged semantics"),
            &missing_proof,
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let mut stale_semantics = decode_module(&semantic_bytes).expect("decode stale semantics");
    let value = stale_semantics.machines[0].parameters[0];
    stale_semantics.machines[0].contract.requires[0] = Proposition::LessOrEqual(
        ScalarTerm::value(value.id, value.scalar_type),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerValue::Unsigned(64),
        )
        .expect("u8 bound"),
    );
    assert!(matches!(
        terminal_verifier::verify_module(
            &stale_semantics,
            &decode_proof_bundle(&proof_bytes).expect("decode unchanged proof"),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::RejectedEvidence {
            obligation: rejected,
            ..
        }) if rejected == obligation
    ));
}
