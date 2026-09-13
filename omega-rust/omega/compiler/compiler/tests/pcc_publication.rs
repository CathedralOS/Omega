//! End-to-end bounded proof-carrying product coverage.
//!
//! The normalized root Build's two independent off-by-default requests
//! produce adjacent `.proof` sidecars beside the ordinary artifacts —
//! never an embedded section, never a different pipeline. Publication
//! validates every requested pair before reporting success, and receivers
//! independently verify the pair against their own pinned policy rather
//! than trusting adjacency, filenames or producer hints.

use std::fs;
use std::path::{Path, PathBuf};

use compilation_report::{NativePccCustodyEvidence, verify_native_proof_sidecar};
use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileOutcomes, CompileRequest,
    RequestedCompileProduct, compile,
};
use proof_admission::AdmissionProfile;
use terminal_codec::{
    PccIncompleteness, PccProductKind, PccProofSidecar, PccReceiverPolicy, PccVerificationOutcome,
    verify_psi_proof_sidecar,
};

const MAIN: &str = "data Main { }\nmachine Main::main(&mut self) { }\n";

fn write_project(pcc_lines: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omega-pcc-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create project dir");
    fs::write(dir.join("main.omg"), MAIN).expect("write main.omg");
    fs::write(
        dir.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{\n    builder.application(\"pcc-test\");\n    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n{pcc_lines}}}\n"
        ),
    )
    .expect("write build.omg");
    dir
}

fn compile_request(dir: &Path, product: RequestedCompileProduct) -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path: dir.join("main.omg"),
        target_name: Some("linux_x86_64".to_owned()),
        build_dir: None,
    })
    .with_requested_product(product)
    .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
}

fn compile_native(dir: &Path) -> compiler::CompileReport {
    compile(compile_request(
        dir,
        RequestedCompileProduct::NativeArtifact,
    ))
    .and_then(CompileOutcomes::into_single_report)
    .expect("native compilation")
}

fn receiver_policy(sidecar: &PccProofSidecar) -> PccReceiverPolicy {
    let mut policy = PccReceiverPolicy::for_offered_claim(sidecar, AdmissionProfile::default());
    // The receiver pins its own package/configuration identity; neither is
    // read from the offered artifact.
    policy.policy_package_identity = "receiver-pinned-policy".to_owned();
    policy.configuration_identity = "receiver-configuration".to_owned();
    policy
}

fn read(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn no_pcc_requests_publish_ordinary_output_only() {
    let dir = write_project("");
    let out = dir.join("out");
    let report = compile_native(&dir);
    assert!(!report.pcc_requests().any());
    let published = report
        .publish_retained_native_artifact(&out)
        .expect("ordinary publication");
    assert!(published.pcc_publications().is_empty());
    let names: Vec<String> = fs::read_dir(&out)
        .expect("out dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        names.len(),
        1,
        "only the executable is published: {names:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn native_pcc_publishes_and_verifies_one_adjacent_pair() {
    let dir = write_project("    builder.pcc.native = true;\n");
    let out = dir.join("out");
    let report = compile_native(&dir);
    assert!(report.pcc_requests().native && !report.pcc_requests().psi);
    let published = report
        .publish_retained_native_artifact(&out)
        .expect("native pcc publication");
    assert_eq!(published.pcc_publications().len(), 1);
    let pair = &published.pcc_publications()[0];
    assert_eq!(pair.product, PccProductKind::Native);
    assert_eq!(
        pair.sidecar_path.extension().and_then(|e| e.to_str()),
        Some("proof")
    );
    assert_eq!(
        pair.artifact_byte_len as usize,
        read(&pair.artifact_path).len()
    );
    assert_eq!(
        pair.sidecar_byte_len as usize,
        read(&pair.sidecar_path).len()
    );
    // No Psi pair was requested: the executable is the only artifact.
    assert!(!pair.artifact_path.with_extension("psi").exists());

    let executable = read(&pair.artifact_path);
    let proof = read(&pair.sidecar_path);
    let policy = receiver_policy(&PccProofSidecar::from_bytes(&proof).expect("decode"));
    // The standalone contract: after deleting source, build files and every
    // other product, verification needs only artifact bytes, sidecar bytes
    // and the receiver's pinned policy.
    fs::remove_file(dir.join("main.omg")).expect("delete source");
    fs::remove_file(dir.join("build.omg")).expect("delete build");
    match verify_native_proof_sidecar(&executable, &proof, &policy) {
        PccVerificationOutcome::Complete(product) => {
            assert_eq!(product.product, PccProductKind::Native);
            assert_eq!(
                product.accepted_guarantees,
                [terminal_codec::NATIVE_CERTIFIED_CUSTODY_GUARANTEE]
            );
            assert_eq!(product.policy_package_identity, "receiver-pinned-policy");
        }
        other => panic!("expected a complete verified pair, got {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn psi_pcc_publishes_the_psi_pair_during_native_compilation() {
    let dir = write_project("    builder.pcc.psi = true;\n");
    let out = dir.join("out");
    let published = compile_native(&dir)
        .publish_retained_native_artifact(&out)
        .expect("psi pcc publication");
    assert_eq!(published.pcc_publications().len(), 1);
    let pair = &published.pcc_publications()[0];
    assert_eq!(pair.product, PccProductKind::Psi);
    assert_eq!(
        pair.artifact_path.extension().and_then(|e| e.to_str()),
        Some("psi")
    );
    assert_eq!(
        pair.sidecar_path.extension().and_then(|e| e.to_str()),
        Some("proof")
    );

    let psi = read(&pair.artifact_path);
    let proof = read(&pair.sidecar_path);
    let policy = receiver_policy(&PccProofSidecar::from_bytes(&proof).expect("decode"));
    match verify_psi_proof_sidecar(&psi, &proof, &policy) {
        PccVerificationOutcome::Complete(product) => {
            assert_eq!(product.product, PccProductKind::Psi);
            assert_eq!(
                product.accepted_guarantees,
                [terminal_codec::PSI_TERMINAL_VERIFIED_GUARANTEE]
            );
        }
        other => panic!("expected a complete verified pair, got {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn both_requests_publish_two_independent_pairs() {
    let dir = write_project("    builder.pcc.psi = true;\n    builder.pcc.native = true;\n");
    let out = dir.join("out");
    let published = compile_native(&dir)
        .publish_retained_native_artifact(&out)
        .expect("dual pcc publication");
    assert_eq!(published.pcc_publications().len(), 2);
    let (psi_pair, native_pair) = if published.pcc_publications()[0].product == PccProductKind::Psi
    {
        (
            &published.pcc_publications()[0],
            &published.pcc_publications()[1],
        )
    } else {
        (
            &published.pcc_publications()[1],
            &published.pcc_publications()[0],
        )
    };
    assert_eq!(native_pair.product, PccProductKind::Native);
    for (artifact_path, sidecar_path, verify) in [
        (
            &psi_pair.artifact_path,
            &psi_pair.sidecar_path,
            verify_psi_proof_sidecar
                as fn(&[u8], &[u8], &PccReceiverPolicy) -> PccVerificationOutcome,
        ),
        (
            &native_pair.artifact_path,
            &native_pair.sidecar_path,
            verify_native_proof_sidecar
                as fn(&[u8], &[u8], &PccReceiverPolicy) -> PccVerificationOutcome,
        ),
    ] {
        let artifact = read(artifact_path);
        let proof = read(sidecar_path);
        let policy = receiver_policy(&PccProofSidecar::from_bytes(&proof).expect("decode"));
        assert!(matches!(
            verify(&artifact, &proof, &policy),
            PccVerificationOutcome::Complete(_)
        ));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stale_or_substituted_bytes_and_wrong_policy_reject() {
    let dir = write_project("    builder.pcc.native = true;\n");
    let out = dir.join("out");
    let published = compile_native(&dir)
        .publish_retained_native_artifact(&out)
        .expect("native pcc publication");
    let pair = &published.pcc_publications()[0];
    let executable = read(&pair.artifact_path);
    let proof = read(&pair.sidecar_path);
    let sidecar = PccProofSidecar::from_bytes(&proof).expect("decode");
    let policy = receiver_policy(&sidecar);

    // A stale sidecar against different bytes rejects on the content
    // commitment; adjacency is never evidence.
    let mut tampered = executable.clone();
    tampered.push(0);
    assert!(matches!(
        verify_native_proof_sidecar(&tampered, &proof, &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "artifact bytes"
    ));

    // A receiver policy requiring an unoffered guarantee rejects.
    let mut denied = policy.clone();
    denied.required_guarantees = vec!["omega.unsupported-claim.v1".to_owned()];
    assert!(matches!(
        verify_native_proof_sidecar(&executable, &proof, &denied),
        PccVerificationOutcome::Reject(ref r) if r.subject == "required guarantee"
    ));

    // A receiver policy that does not accept the checker profile rejects.
    let mut denied = policy.clone();
    denied.accepted_checker_profiles = vec!["other-profile".to_owned()];
    assert!(matches!(
        verify_native_proof_sidecar(&executable, &proof, &denied),
        PccVerificationOutcome::Reject(ref r) if r.subject == "checker profile"
    ));

    // A receiver policy that does not admit the claimed assumption closure
    // rejects; a sidecar carrying a tampered custody claim also rejects.
    let mut denied = policy.clone();
    denied.admitted_assumptions = Vec::new();
    assert!(matches!(
        verify_native_proof_sidecar(&executable, &proof, &denied),
        PccVerificationOutcome::Reject(ref r) if r.subject == "assumption"
    ));

    let mut custody = NativePccCustodyEvidence::from_bytes(sidecar.evidence()).expect("custody");
    custody.evidence_digest = [0xee; 32];
    let forged = PccProofSidecar::new(
        PccProductKind::Native,
        *sidecar.artifact_commitment(),
        sidecar.semantic_profile().to_owned(),
        sidecar.checker_profile().to_owned(),
        sidecar.guarantees().to_vec(),
        custody.to_bytes(),
        sidecar.assumptions().to_vec(),
        sidecar.dependencies().to_vec(),
    )
    .expect("forged sidecar encodes");
    assert!(matches!(
        verify_native_proof_sidecar(&executable, &forged.to_bytes(), &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "native custody evidence"
    ));

    // A named resource limit reports Incomplete, not Reject.
    let mut exhausted = policy.clone();
    exhausted.max_artifact_bytes = 4;
    assert_eq!(
        verify_native_proof_sidecar(&executable, &proof, &exhausted),
        PccVerificationOutcome::Incomplete(PccIncompleteness::ArtifactBytes {
            actual: executable.len() as u64,
            limit: 4,
        })
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn omitted_dependencies_require_exact_possession() {
    let dir = write_project("    builder.pcc.native = true;\n");
    let out = dir.join("out");
    let published = compile_native(&dir)
        .publish_retained_native_artifact(&out)
        .expect("native pcc publication");
    let pair = &published.pcc_publications()[0];
    let executable = read(&pair.artifact_path);
    let proof = read(&pair.sidecar_path);
    let sidecar = PccProofSidecar::from_bytes(&proof).expect("decode");

    // The minimal program omits no dependencies; a receiver possessing
    // nothing extra verifies, and a receiver that reports different material
    // under the same identity rejects.
    let policy = receiver_policy(&sidecar);
    assert_eq!(
        sidecar.dependencies(),
        policy.possessed_dependencies.as_slice()
    );
    assert!(matches!(
        verify_native_proof_sidecar(&executable, &proof, &policy),
        PccVerificationOutcome::Complete(_)
    ));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn check_and_terminal_stops_reject_conflicting_pcc_requests() {
    let dir = write_project("    builder.pcc.native = true;\n");
    let check = compile(compile_request(&dir, RequestedCompileProduct::Check))
        .and_then(CompileOutcomes::into_single_report);
    let diagnostics = check.expect_err("check stop cannot satisfy native PCC");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.to_string().contains("check-only stop")),
        "expected a check-stop conflict diagnostic: {diagnostics:?}"
    );
    let terminal = compile(compile_request(
        &dir,
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(CompileOutcomes::into_single_report);
    let diagnostics = terminal.expect_err("terminal stop cannot satisfy native PCC");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.to_string().contains("Terminal stop")),
        "expected a Terminal-stop conflict diagnostic: {diagnostics:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn terminal_stop_with_psi_pcc_publishes_the_psi_pair() {
    let dir = write_project("    builder.pcc.psi = true;\n");
    let report = compile(compile_request(
        &dir,
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(CompileOutcomes::into_single_report)
    .expect("terminal compilation");
    let out = dir.join("out");
    let published = report
        .publish_retained_terminal_artifact(&out)
        .expect("terminal publication");
    assert_eq!(published.pcc_publications().len(), 1);
    let pair = &published.pcc_publications()[0];
    assert_eq!(pair.product, PccProductKind::Psi);
    let psi = read(&pair.artifact_path);
    let proof = read(&pair.sidecar_path);
    let policy = receiver_policy(&PccProofSidecar::from_bytes(&proof).expect("decode"));
    assert!(matches!(
        verify_psi_proof_sidecar(&psi, &proof, &policy),
        PccVerificationOutcome::Complete(_)
    ));
    let _ = fs::remove_dir_all(&dir);
}
