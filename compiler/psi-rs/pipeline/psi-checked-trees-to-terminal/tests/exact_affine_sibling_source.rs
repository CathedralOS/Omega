use psi_proof_kernel::{AdmissionProfile, EvidenceRoute, ProofRule};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::OperationKind;
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(token: Token, root: i8) -> bool
    requires 1i8 <= root
    {
        root < 1i8 || (6i8 / (root * 1i8)) <= 6i8
    }
"#;

#[test]
fn landed_affine_sibling_custody_crosses_source_codec_and_independent_verification() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("the landed affine sibling completes the source certificate");

    let exact_divide = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("source retains one exact divide");
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == exact_divide)
        .expect("exact divide has evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
        panic!("the exact divide uses the canonical certificate route")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, index: 1 } = &certificate.proof.rule else {
        panic!("the nonnegative root selects the signed positive-divisor arm")
    };
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("the positive-divisor arm retains its affine custody")
    };
    assert_eq!(witness.definition_axioms.len(), 1);
    assert_eq!(witness.literal_axioms.len(), 1);
    assert!(witness.literal_axioms[0].is_some());

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verification replays the landed sibling");

    let module_bytes = encode_module(&lowered.semantic_module).expect("encode module");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof bundle");
    let decoded_module = decode_module(&module_bytes).expect("decode module");
    let decoded_proof = decode_proof_bundle(&proof_bytes).expect("decode proof bundle v19");
    assert_eq!(decoded_module, lowered.semantic_module);
    assert_eq!(decoded_proof, lowered.proof_bundle);

    let mut stale = decoded_proof;
    let stale_evidence = stale
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == exact_divide)
        .expect("decoded exact-divide evidence");
    let EvidenceRoute::CertificateDerived(certificate) = &mut stale_evidence.route else {
        unreachable!("selected certificate-derived evidence")
    };
    let ProofRule::DisjunctionIntroduction { disjunct, .. } = &mut certificate.proof.rule else {
        unreachable!("selected signed disjunction proof")
    };
    let ProofRule::IntegerAffineBound { witness, .. } = &mut disjunct.rule else {
        unreachable!("selected affine child")
    };
    witness.literal_axioms[0] = None;
    assert!(
        psi_terminal_verifier::verify_module(
            &decoded_module,
            &stale,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "removing the landed-sibling citation invalidates the certificate",
    );
}
