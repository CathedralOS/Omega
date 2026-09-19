use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use compiler::{CheckedCompileRequest, compile_to_checked};
use proof_admission::{
    Budget, Term, certificate_assumption_closure,
    verify_bounded_certificate_with_machine_parameters, verify_mathematical_certificate,
};
use terminal_psi::{EvidenceRoute, ProofNode, ProofRule};

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/omega")
        .join(relative)
        .join("main.omg")
}

fn subtraction(proof: &ProofNode) -> Option<&ProofNode> {
    match &proof.rule {
        ProofRule::IntegerSubtractOrder { .. } => Some(proof),
        ProofRule::IntegerOrderSubstitution {
            relation, equality, ..
        } => subtraction(relation).or_else(|| subtraction(equality)),
        ProofRule::ConjunctionIntroduction(children) => children.iter().find_map(subtraction),
        _ => None,
    }
}

#[test]
fn source_rank_decrease_has_checked_subtraction_evidence() {
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &fixture("pass/proofs/kernel_integer_subtract_order"),
        Some("linux_x86_64"),
    ))
    .unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "walk")
        .produce_artifact()
        .unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let questions = terminal_verifier::reconstruct_control_cycle_obligations(&module).unwrap();
    let validated = terminal_verifier::validate_module_for_interpretation(&module).unwrap();
    let mut witnessed = false;
    for cycle in &proof.control_cycles {
        let question = questions
            .iter()
            .find(|question| question.component == cycle.component)
            .unwrap();
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == question.machine)
            .unwrap();
        let context = validated.value_context(machine).unwrap();
        let parameters = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect();
        for edge in &cycle.certificate.edges {
            let EvidenceRoute::CertificateDerived(certificate) = &edge.evidence else {
                continue;
            };
            let Some(subtraction) = subtraction(&certificate.proof) else {
                continue;
            };
            witnessed = true;
            let obligation = &question
                .obligation
                .edges
                .iter()
                .find(|candidate| candidate.decrease.obligation.id == edge.obligation)
                .unwrap()
                .decrease;
            let denoted = verify_bounded_certificate_with_machine_parameters(
                &context,
                &subtraction.conclusion,
                &obligation.assumptions,
                &obligation.semantic_axioms,
                &parameters,
                subtraction,
                &mut Budget::default(),
            )
            .unwrap();
            eprintln!("subtraction source receipt: {:?}", denoted.receipt());
            for declaration in &denoted.certificate.signature {
                if declaration.body.is_some() {
                    continue;
                }
                let mut conclusion = declaration.ty;
                while let Term::Pi { codomain, .. } = denoted.arena.get(conclusion) {
                    conclusion = codomain;
                }
                assert!(
                    !denoted
                        .arena
                        .structurally_equal(conclusion, denoted.certificate.expected),
                    "the source conclusion must be derived rather than assumed by a rule-instance axiom"
                );
            }
            // Recheck the complete reconstructed edge, including the SSA
            // equality transports surrounding the subtraction node.
            let denoted = verify_bounded_certificate_with_machine_parameters(
                &context,
                &obligation.obligation.proposition,
                &obligation.assumptions,
                &obligation.semantic_axioms,
                &parameters,
                &certificate.proof,
                &mut Budget::default(),
            )
            .unwrap();
            let closure = certificate_assumption_closure(&denoted.arena, &denoted.certificate);
            // The entire reconstructed context is part of the judgment:
            // its value vocabulary remains in the closure alongside the
            // fixed subtraction/order laws. Three numeral definitions are
            // checked bodies rather than assumptions. No rule-instance
            // conclusion is admitted.
            assert_eq!(
                closure,
                (0..51)
                    .filter(|position| ![5, 7, 8].contains(position))
                    .collect::<BTreeSet<_>>()
            );
            eprintln!("edge closure={closure:?}; receipt={:?}", denoted.receipt());
            let bytes = terminal_codec::encode_mathematical_certificate(
                &denoted.arena,
                &denoted.certificate,
            )
            .unwrap();
            let mut decoded = terminal_codec::decode_mathematical_certificate(&bytes).unwrap();
            verify_mathematical_certificate(
                &mut decoded.arena,
                &decoded.certificate,
                &mut Budget::default(),
            )
            .unwrap();
            assert_eq!(
                certificate_assumption_closure(&decoded.arena, &decoded.certificate),
                closure
            );
            assert_eq!(
                terminal_codec::encode_mathematical_certificate(
                    &decoded.arena,
                    &decoded.certificate
                )
                .unwrap(),
                bytes
            );
            assert!(
                verify_bounded_certificate_with_machine_parameters(
                    &context,
                    &obligation.obligation.proposition,
                    &[],
                    &[],
                    &parameters,
                    &certificate.proof,
                    &mut Budget::default(),
                )
                .is_err()
            );
        }
    }
    assert!(
        witnessed,
        "the actual ranked loop must retain IntegerSubtractOrder"
    );
}

#[test]
fn source_non_decreasing_rank_rejects() {
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &fixture("fail/proofs/kernel_integer_subtract_order_false_twin"),
        Some("linux_x86_64"),
    ))
    .expect_err("zero decrement cannot establish strict rank decrease");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("cannot prove the `terminates by` ranking")),
        "{diagnostics:#?}"
    );
}
