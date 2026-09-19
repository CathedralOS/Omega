use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use proof_admission::{
    Budget, certificate_assumption_closure, verify_bounded_certificate,
    verify_mathematical_certificate,
};
use terminal_psi::{EvidenceRoute, ProofRule};

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/omega")
        .join(relative)
        .join("main.omg")
}

fn request(relative: &str) -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path: fixture(relative),
        build_dir: None,
        target_name: Some("linux_x86_64".to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact)
}

#[test]
fn source_discreteness_reaches_a_retained_checked_certificate() {
    let report = compile(request("pass/proofs/kernel_integer_discreteness"))
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("source discreteness: {diagnostics:#?}"));
    let artifact = report.artifact().expect("Terminal artifact");
    artifact
        .validate()
        .expect("independent Terminal validation");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let (evidence, certificate) = proof
        .evidence
        .iter()
        .find_map(|evidence| {
            let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
                return None;
            };
            contains_discreteness(&certificate.proof).then_some((evidence, certificate))
        })
        .unwrap_or_else(|| {
            panic!("source must produce discreteness, not just compile: {proof:#?}")
        });
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let validated = terminal_verifier::validate_module(&module).unwrap();
    let obligations =
        terminal_verifier::reconstruct_execution_terminal_obligations(validated).unwrap();
    let site = obligations
        .obligations()
        .iter()
        .find(|site| site.obligation.id == evidence.obligation)
        .unwrap();
    let machine = validated.machine(site.owner.machine()).unwrap();
    let context = validated.value_context(machine).unwrap();
    let denoted = verify_bounded_certificate(
        &context,
        &site.obligation.proposition,
        &site.requirements,
        &site.semantic_axioms,
        &certificate.proof,
        &mut Budget::default(),
    )
    .unwrap();
    let closure = certificate_assumption_closure(&denoted.arena, &denoted.certificate);
    // Int, both relations, zero/double/odd, two symbolic values, three
    // numeral laws and three order/substitution laws. All numeral prefix
    // definitions and the unused negate declaration stay out of this exact
    // assumption closure. No per-instance strict conclusion is assumed.
    assert_eq!(
        closure,
        BTreeSet::from([0, 1, 2, 3, 4, 8, 12, 13, 15, 17, 18, 19, 20, 21])
    );
    eprintln!("source closure={closure:?} receipt={:?}", denoted.receipt());
    let bytes =
        terminal_codec::encode_mathematical_certificate(&denoted.arena, &denoted.certificate)
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
        terminal_codec::encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .unwrap(),
        bytes
    );
    // Evidence without the reconstructed entry contract cannot establish the
    // strict call requirement, even though all numeral laws remain available.
    assert!(
        verify_bounded_certificate(
            &context,
            &site.obligation.proposition,
            &[],
            &site.semantic_axioms,
            &certificate.proof,
            &mut Budget::default()
        )
        .is_err()
    );
}

fn contains_discreteness(proof: &terminal_psi::ProofNode) -> bool {
    match &proof.rule {
        ProofRule::IntegerOrderDiscreteness { .. } => true,
        ProofRule::IntegerOrderSubstitution {
            relation, equality, ..
        } => contains_discreteness(relation) || contains_discreteness(equality),
        _ => false,
    }
}

#[test]
fn source_discreteness_false_twin_rejects_the_inclusive_endpoint() {
    let diagnostics = compile(request(
        "fail/proofs/kernel_integer_discreteness_false_twin",
    ))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect_err("value == 8 does not imply value < 8");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("cannot prove") && rendered.contains("requires"),
        "{rendered}"
    );
}
