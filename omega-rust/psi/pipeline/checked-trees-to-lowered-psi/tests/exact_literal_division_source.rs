use proof_admission::{AdmissionProfile, EvidenceRoute, ProofRule};
use semantic_vocabulary::{IntegerValue, Proposition};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_psi::OperationKind;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    machine enter() -> bool
    requires true == true
    ensures true == true
    {
        let dividend: i8 = -7i8;
        dividend / -1i8 == 7i8 && dividend % -1i8 == 0i8
    }
"#;

#[test]
fn landed_negative_one_and_nonminimum_dividend_use_closed_exact_certificates() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "enter")
        .expect("closed signed exact divide/remainder source lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let exact_obligations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { obligation, .. }
            | OperationKind::ExactIntegerRemainder { obligation, .. } => Some(obligation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(exact_obligations.len(), 2, "one divide and one remainder");
    for obligation in exact_obligations {
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == obligation)
            .expect("exact operation has evidence");
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("closed signed exact operation has a recursive certificate")
        };
        let Proposition::Disjunction(disjuncts) = &certificate.proof.conclusion else {
            panic!("signed exact division uses the canonical three-arm definedness goal")
        };
        assert_eq!(disjuncts.len(), 3);
        let ProofRule::DisjunctionIntroduction { disjunct, index } = &certificate.proof.rule else {
            panic!("landed -1 and nonminimum dividend select the exceptional arm")
        };
        assert_eq!(*index, 2);
        assert_eq!(disjunct.conclusion, disjuncts[2]);
        assert!(matches!(
            disjunct.rule,
            ProofRule::ConjunctionIntroduction(ref conjuncts) if conjuncts.len() == 2
        ));
    }

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier accepts both canonical certificates");
    let encoded_module = encode_module(&lowered.semantic_module).expect("encode module");
    let encoded_proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof bundle");
    let decoded_module = decode_module(&encoded_module).expect("decode module");
    let decoded_proof = decode_proof_bundle(&encoded_proof).expect("decode proof bundle");
    assert_eq!(decoded_module, lowered.semantic_module);
    assert_eq!(decoded_proof, lowered.proof_bundle);

    let mut minimum_dividend = decoded_module;
    let landed_dividend = minimum_dividend
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Signed(-7),
                }
            )
        })
        .expect("source retains the landed dividend literal");
    landed_dividend.kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(-128),
    };
    assert!(
        terminal_verifier::verify_module(
            &minimum_dividend,
            &decoded_proof,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "stale closed-goal certificates reject a minimum dividend",
    );
}
