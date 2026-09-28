use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::{AdmissionProfile, EvidenceRoute, ProofNode, ProofRule};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_psi::OperationKind;

const SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(token: Token) -> u16 {
        let r: u16 = 3u16;
        let a: u16 = 5u16;
        let b: u16 = 7u16;
        let inner: u16 = a + b;
        let middle: u16 = r + inner;
        let bridge: u16 = b + r;
        let join: u16 = middle + bridge;
        r + join
    }
"#;

#[test]
fn exact_outer_fork_join_crosses_source_codec_and_independent_verification() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("bounded affine-chain endpoint conjunction lowers");

    let final_add = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { obligation, .. } => Some(obligation),
            _ => None,
        })
        .next_back()
        .expect("source retains the final exact add");
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == final_add)
        .expect("final exact add has evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
        panic!("final exact add uses canonical certificate custody")
    };
    assert_single_computed_join_conjunction(&certificate.proof);

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verification replays source-produced conjunction custody");

    let module_bytes = encode_module(&lowered.semantic_module).expect("encode module");
    let proof_bytes = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("encode proof bundle");
    let decoded_module = decode_module(&module_bytes).expect("decode module");
    let decoded_proof = decode_proof_bundle(&proof_bytes).expect("decode proof bundle");
    assert_eq!(decoded_module, lowered.semantic_module);
    assert_eq!(decoded_proof, lowered.proof_bundle);
    terminal_verifier::verify_module(
        &decoded_module,
        &decoded_proof,
        &AdmissionProfile::default(),
    )
    .expect("independent verification replays decoded conjunction custody");
}

fn assert_single_computed_join_conjunction(proof: &ProofNode) {
    let mapped = match &proof.rule {
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            ..
        } => left_less_or_equal_middle.as_ref(),
        _ => proof,
    };
    let ProofRule::IntegerAffineBound { root_bound, .. } = &mapped.rule else {
        panic!("final add retains its checked direct affine map")
    };
    let ProofRule::ConjunctionIntroduction(endpoints) = &root_bound.rule else {
        panic!("final add retains two ordered endpoint proofs")
    };
    assert_eq!(endpoints.len(), 2);
    // The upper endpoint may arrive relaxed through a transitivity hop: the
    // derived-equality closure proves the tight `join` bound first, then
    // relaxes it to the goal endpoint. Unwrap that hop before checking the
    // certificate shape.
    let upper_endpoint = match &endpoints[1].rule {
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            ..
        } => left_less_or_equal_middle.as_ref(),
        _ => &endpoints[1],
    };
    assert!(matches!(
        upper_endpoint.rule,
        ProofRule::IntegerExactAddDefinitionBound {
            definition_axiom: 6,
            ..
        }
    ));
}
