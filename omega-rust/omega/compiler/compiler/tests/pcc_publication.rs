//! End-to-end bounded proof-carrying product coverage.
//!
//! The normalized root Build's two independent off-by-default requests
//! retain Psi proof pairs and report unsupported native proof requests —
//! never an embedded section, never a different pipeline. Publication
//! validates every requested pair before reporting success, and receivers
//! independently verify the pair against their own pinned policy rather
//! than trusting adjacency, filenames or producer hints.

use std::fs;
use std::path::{Path, PathBuf};

use compilation_report::verify_native_proof_sidecar;
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

// Reproduce the obsolete custody protocol without trusting its producer API.
// Every native digest is attacker-chosen; a valid unrelated Psi artifact is
// enough to recompute all of the consistency hashes for arbitrary bytes.
fn forged_native_sidecar(psi_bytes: &[u8], executable: &[u8]) -> PccProofSidecar {
    use sha2::{Digest, Sha256};
    let psi = terminal_codec::CanonicalTerminalArtifact::from_bytes(psi_bytes).expect("psi");
    let mut fields = vec![0; 32]; // Native artifact identity.
    fields.extend_from_slice(&[2, 1]); // x86-64 ELF.
    fields.extend_from_slice(&8_u64.to_le_bytes());
    fields.extend_from_slice(&8_u64.to_le_bytes());
    fields.extend_from_slice(&[0; 32]); // Image symbol digest.
    fields.extend_from_slice(&[0; 9]); // No boundary fingerprint.
    fields.extend_from_slice(&[0; 32]); // Text validation digest.
    fields.extend_from_slice(&[0; 32]); // Function validation digest.
    fields.extend_from_slice(&[0; 8]); // Function report fingerprint.
    fields.extend_from_slice(&[0; 32]); // Inventory digest.
    fields.extend_from_slice(&[0; 8]); // Inventory report fingerprint.

    let mut certificate = Sha256::new();
    certificate.update(b"omega.native-publication-certificate.sha256.v1\0");
    certificate.update(&fields[..32]);
    for bytes in [psi.semantic_bytes(), psi.proof_bytes()] {
        certificate.update((bytes.len() as u64).to_le_bytes());
        certificate.update(bytes);
    }
    certificate.update(&fields[32..]);
    let certificate = certificate.finalize();

    let mut container = Sha256::new();
    container.update(b"omega.published-executable-container.sha256.v1\0");
    container.update((executable.len() as u64).to_le_bytes());
    container.update(executable);

    let mut evidence = Sha256::new();
    evidence.update(b"omega.native-publication-evidence.sha256.v1\0");
    evidence.update([0; 32]);
    evidence.update(certificate);
    evidence.update([0; 64]); // Function and inventory digests.
    evidence.update([0; 24]); // Callback, inventory and function reports.
    evidence.update((executable.len() as u64).to_le_bytes());
    evidence.update([0; 32]); // Text validation digest.
    evidence.update(container.finalize());

    let mut custody = 1_u16.to_le_bytes().to_vec();
    custody.extend_from_slice(&(psi_bytes.len() as u64).to_le_bytes());
    custody.extend_from_slice(psi_bytes);
    custody.extend_from_slice(&fields);
    custody.extend_from_slice(&[0; 8]); // Callback report fingerprint.
    custody.extend_from_slice(&certificate);
    custody.extend_from_slice(&evidence.finalize());
    PccProofSidecar::new(
        PccProductKind::Native,
        terminal_codec::pcc_artifact_commitment(executable),
        format!(
            "native-custody-v1/{}",
            terminal_codec::psi_semantic_profile_identity(&psi).expect("profile")
        ),
        terminal_codec::admission_profile_identity(&AdmissionProfile::default()),
        vec![terminal_codec::PccGuarantee {
            identity: "omega.native-certified-custody.v1".to_owned(),
            premises: Vec::new(),
        }],
        custody,
        terminal_codec::terminal_assumption_closure(),
        Vec::new(),
    )
    .expect("forged sidecar")
}

#[test]
fn arbitrary_native_bytes_with_recomputed_custody_never_complete() {
    let dir = write_project("    builder.pcc.psi = true;\n");
    let report = compile(compile_request(
        &dir,
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(CompileOutcomes::into_single_report)
    .expect("terminal compilation");
    let published = report
        .publish_retained_terminal_artifact(&dir.join("out"))
        .expect("psi publication");
    let psi = read(&published.pcc_publications()[0].artifact_path);
    let executable = b"arbitrary bytes, not an executable";
    let sidecar = forged_native_sidecar(&psi, executable);
    let outcome =
        verify_native_proof_sidecar(executable, &sidecar.to_bytes(), &receiver_policy(&sidecar));
    assert_eq!(
        outcome,
        PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
            product: PccProductKind::Native
        })
    );
    let _ = fs::remove_dir_all(&dir);
}

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
fn native_pcc_is_incomplete_before_publication() {
    let dir = write_project("    builder.pcc.native = true;\n");
    let out = dir.join("out");
    let report = compile_native(&dir);
    assert!(report.pcc_requests().native && !report.pcc_requests().psi);
    let failure = report
        .publish_retained_native_artifact(&out)
        .expect_err("native PCC requires unavailable native semantics evidence");
    assert!(
        failure.contains("Incomplete(UnsupportedEvidence { product: Native })"),
        "{failure}"
    );
    assert!(
        !out.exists(),
        "unsupported publication must not create output"
    );
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
fn both_requests_are_incomplete_without_partial_publication() {
    let dir = write_project("    builder.pcc.psi = true;\n    builder.pcc.native = true;\n");
    let out = dir.join("out");
    fs::create_dir_all(&out).expect("output directory");
    let previous = out.join("previous-output");
    fs::write(&previous, b"retained output").expect("previous output");
    let failure = compile_native(&dir)
        .publish_retained_native_artifact(&out)
        .expect_err("both requests cannot complete while native PCC is unsupported");
    assert!(
        failure.contains("Incomplete(UnsupportedEvidence { product: Native })"),
        "{failure}"
    );
    assert_eq!(read(&previous), b"retained output");
    assert_eq!(fs::read_dir(&out).expect("output directory").count(), 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stale_or_substituted_bytes_and_wrong_policy_reject() {
    let dir = write_project("    builder.pcc.psi = true;\n");
    let published = compile(compile_request(
        &dir,
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(CompileOutcomes::into_single_report)
    .expect("terminal compilation")
    .publish_retained_terminal_artifact(&dir.join("out"))
    .expect("psi publication");
    let psi = read(&published.pcc_publications()[0].artifact_path);
    let executable = b"arbitrary bytes, not an executable".to_vec();
    let sidecar = forged_native_sidecar(&psi, &executable);
    let proof = sidecar.to_bytes();
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
    // rejects before unsupported native evidence is considered.
    let mut denied = policy.clone();
    denied.admitted_assumptions = Vec::new();
    assert!(matches!(
        verify_native_proof_sidecar(&executable, &proof, &denied),
        PccVerificationOutcome::Reject(ref r) if r.subject == "assumption"
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
fn offered_native_profiles_never_grant_assurance() {
    for profile in [
        "native-custody-v1",
        "native-semantics-v999",
        "terminal-psi-vocabulary-96",
    ] {
        let executable = b"not executable";
        let sidecar = PccProofSidecar::new(
            PccProductKind::Native,
            terminal_codec::pcc_artifact_commitment(executable),
            profile.to_owned(),
            terminal_codec::admission_profile_identity(&AdmissionProfile::default()),
            vec![terminal_codec::PccGuarantee {
                identity: "omega.standard-memory-safety".to_owned(),
                premises: Vec::new(),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("sidecar");
        assert_eq!(
            verify_native_proof_sidecar(
                executable,
                &sidecar.to_bytes(),
                &receiver_policy(&sidecar)
            ),
            PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
                product: PccProductKind::Native,
            }),
        );
    }
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
