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

use compilation_report::{verify_native_proof_sidecar, verify_published_proof_pair};
use compiler::{CompileOptions, CompileOutcomes, CompileRequest, RequestedCompileProduct, compile};
use proof_admission::AdmissionProfile;
use terminal_codec::{
    PccIncompleteness, PccProductKind, PccProofSidecar, PccReceiverPolicy, PccVerificationOutcome,
    verify_psi_proof_sidecar,
};

const MAIN: &str = "data Main { }\nmachine Main::main(&mut self) { }\n";

// The bound entry invokes a top-level boundary requirement whose reach stays
// installation-bound, so the compiled module — and therefore the published
// sidecar — carries a real omitted-dependency inventory rather than a fixture
// injected after compilation.
const DEPENDENCY_MAIN: &str = "pub boundary trait Console {}\n\
     pub data Endpoint {}\n\
     pub boundary requirement Endpoint::step() invokes Console; reaches <= Console;\n\
     data Main { }\n\
     machine Main::main(&mut self) invokes Console; { Endpoint::step(); }\n";

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

fn build_source(body_lines: &str) -> String {
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"pcc-test\");\n    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n{body_lines}}}\n"
    )
}

fn write_project(pcc_lines: &str) -> PathBuf {
    write_project_source(MAIN, pcc_lines)
}

fn write_project_source(main: &str, pcc_lines: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omega-pcc-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create project dir");
    fs::write(dir.join("main.omg"), main).expect("write main.omg");
    fs::write(dir.join("build.omg"), build_source(pcc_lines)).expect("write build.omg");
    dir
}

// A receiver-less `main` avoids hosted-receiver custody provisioning on the
// macOS ARM64 `ProgramEntry`, matching the GUI publication fixtures.
const GUI_MAIN: &str = "data Main { }\nmachine Main::main() { }\n";

fn write_gui_project() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omega-pcc-gui-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create project dir");
    fs::write(dir.join("main.omg"), GUI_MAIN).expect("write main.omg");
    fs::write(
        dir.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.application(\"pcc-gui\");\n    builder.subsystem = Subsystem::Gui;\n    builder.identifier = \"com.omega.pcc-gui\";\n    builder.pcc.psi = true;\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);\n}\n",
    )
    .expect("write build.omg");
    dir
}

fn compile_request(dir: &Path, product: RequestedCompileProduct) -> CompileRequest {
    compile_request_for(dir, "linux_x86_64", product)
}

fn compile_request_for(
    dir: &Path,
    target: &str,
    product: RequestedCompileProduct,
) -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path: dir.join("main.omg"),
        target_name: Some(target.to_owned()),
        build_dir: None,
    })
    .with_requested_product(product)
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
    // Producer and receiver report the artifact and companion sizes separately.
    assert_eq!(
        pair.artifact_byte_len,
        fs::metadata(&pair.artifact_path)
            .expect("artifact metadata")
            .len()
    );
    assert_eq!(
        pair.sidecar_byte_len,
        fs::metadata(&pair.sidecar_path)
            .expect("sidecar metadata")
            .len()
    );
    assert_ne!(pair.artifact_byte_len, pair.sidecar_byte_len);

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

    // A guarantee premise the receiver does not admit rejects.
    let mut premised_guarantees = sidecar.guarantees().to_vec();
    premised_guarantees[0]
        .premises
        .push("receiver-unadmitted-premise".to_owned());
    let premised = PccProofSidecar::new(
        sidecar.product(),
        *sidecar.artifact_commitment(),
        sidecar.semantic_profile().to_owned(),
        sidecar.checker_profile().to_owned(),
        premised_guarantees,
        sidecar.evidence().to_vec(),
        sidecar.assumptions().to_vec(),
        sidecar.dependencies().to_vec(),
    )
    .expect("premised sidecar");
    assert!(matches!(
        verify_native_proof_sidecar(&executable, &premised.to_bytes(), &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "guarantee premise"
    ));

    // An omitted dependency the receiver does not independently possess
    // rejects by exact identity, not by proximity or version.
    let dependent = PccProofSidecar::new(
        sidecar.product(),
        *sidecar.artifact_commitment(),
        sidecar.semantic_profile().to_owned(),
        sidecar.checker_profile().to_owned(),
        sidecar.guarantees().to_vec(),
        sidecar.evidence().to_vec(),
        sidecar.assumptions().to_vec(),
        vec![terminal_codec::PccDependency {
            identity: "test::missing-material".to_owned(),
            content_commitment: [3; 32],
        }],
    )
    .expect("dependent sidecar");
    assert!(matches!(
        verify_native_proof_sidecar(&executable, &dependent.to_bytes(), &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "dependency"
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

#[test]
fn macos_gui_psi_pcc_installs_the_inner_sidecar_pair() {
    // Real bundle placement: the requested Psi pair sits beside the inner
    // Contents/MacOS executable, and the receipt names those installed paths
    // with separate byte sizes (wiki/spec/proofs/publication.md).
    let dir = write_gui_project();
    let out = dir.join("out");
    let published = compile(compile_request_for(
        &dir,
        "macos_arm64",
        RequestedCompileProduct::NativeArtifact,
    ))
    .and_then(CompileOutcomes::into_single_report)
    .expect("macOS GUI compilation")
    .publish_retained_native_artifact(&out)
    .expect("gui publication installs one .app package");
    let package_root = published
        .checked_native_package_path()
        .expect("checked package root")
        .to_path_buf();
    assert_eq!(package_root, out.join("pcc-gui.app"));
    assert_eq!(
        published
            .checked_native_executable_path()
            .expect("checked inner executable"),
        package_root.join("Contents/MacOS/pcc-gui").as_path()
    );
    let [pair] = published.pcc_publications() else {
        panic!("expected exactly one published pair")
    };
    assert_eq!(pair.product, PccProductKind::Psi);
    let macos_dir = package_root.join("Contents").join("MacOS");
    assert_eq!(pair.artifact_path, macos_dir.join("pcc-gui.psi"));
    assert_eq!(pair.sidecar_path, macos_dir.join("pcc-gui.psi.proof"));
    assert_eq!(
        pair.artifact_byte_len,
        fs::metadata(&pair.artifact_path)
            .expect("artifact metadata")
            .len()
    );
    assert_eq!(
        pair.sidecar_byte_len,
        fs::metadata(&pair.sidecar_path)
            .expect("sidecar metadata")
            .len()
    );
    // The installed pair is a genuine certified product, not just staged bytes.
    let psi = read(&pair.artifact_path);
    let proof = read(&pair.sidecar_path);
    let policy = receiver_policy(&PccProofSidecar::from_bytes(&proof).expect("decode"));
    match verify_psi_proof_sidecar(&psi, &proof, &policy) {
        PccVerificationOutcome::Complete(product) => {
            assert_eq!(product.product, PccProductKind::Psi);
        }
        other => panic!("expected a complete verified inner pair, got {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn republishing_without_pcc_removes_stale_companions() {
    // A previous Psi-requested publication's companions must not stay bound
    // to bytes they do not commit to: turning the request off and publishing
    // again removes them rather than leaving a stale association.
    let dir = write_project("    builder.pcc.psi = true;\n");
    let out = dir.join("out");
    let published = compile_native(&dir)
        .publish_retained_native_artifact(&out)
        .expect("psi pcc publication");
    let pair = &published.pcc_publications()[0];
    let stale_psi = pair.artifact_path.clone();
    let stale_proof = pair.sidecar_path.clone();
    let executable = published
        .checked_native_executable_path()
        .expect("executable")
        .to_path_buf();
    assert!(stale_psi.is_file() && stale_proof.is_file());

    fs::write(dir.join("build.omg"), build_source("")).expect("rewrite build.omg");
    let republished = compile_native(&dir)
        .publish_retained_native_artifact(&out)
        .expect("ordinary publication");
    assert!(republished.pcc_publications().is_empty());
    assert!(
        !stale_proof.exists(),
        "stale .proof must not survive beside new executable bytes"
    );
    assert!(
        !stale_psi.exists(),
        "stale .psi companion must not survive beside new executable bytes"
    );
    assert!(executable.is_file());
    assert_eq!(fs::read_dir(&out).expect("out dir").count(), 1);

    // The Terminal product follows the same rule for its own `.proof`.
    let terminal_dir = write_project("    builder.pcc.psi = true;\n");
    let terminal_out = terminal_dir.join("out");
    let first = compile(compile_request(
        &terminal_dir,
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(CompileOutcomes::into_single_report)
    .expect("terminal compilation")
    .publish_retained_terminal_artifact(&terminal_out)
    .expect("first terminal publication");
    let terminal_proof = first.pcc_publications()[0].sidecar_path.clone();
    assert!(terminal_proof.is_file());
    fs::write(terminal_dir.join("build.omg"), build_source("")).expect("rewrite build.omg");
    let second = compile(compile_request(
        &terminal_dir,
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(CompileOutcomes::into_single_report)
    .expect("terminal recompilation")
    .publish_retained_terminal_artifact(&terminal_out)
    .expect("terminal republication");
    assert!(second.pcc_publications().is_empty());
    assert!(
        !terminal_proof.exists(),
        "stale .proof must not survive beside new artifact bytes"
    );
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&terminal_dir);
}

/// Compile `main` with the Psi proof request and publish the Terminal pair,
/// returning the project directory and the installed artifact/companion bytes.
fn publish_psi_pair(main: &str) -> (PathBuf, Vec<u8>, Vec<u8>) {
    let dir = write_project_source(main, "    builder.pcc.psi = true;\n");
    let published = compile(compile_request(
        &dir,
        RequestedCompileProduct::TerminalArtifact,
    ))
    .and_then(CompileOutcomes::into_single_report)
    .expect("terminal compilation")
    .publish_retained_terminal_artifact(&dir.join("out"))
    .expect("psi publication");
    let pair = &published.pcc_publications()[0];
    (dir, read(&pair.artifact_path), read(&pair.sidecar_path))
}

/// Rebuild one offered sidecar with selected fields replaced, preserving the
/// canonical ordering rules construction enforces.
fn rebuild_sidecar(
    sidecar: &PccProofSidecar,
    product: PccProductKind,
    artifact_commitment: [u8; 32],
    guarantees: Vec<terminal_codec::PccGuarantee>,
    evidence: Vec<u8>,
    assumptions: Vec<String>,
    dependencies: Vec<terminal_codec::PccDependency>,
) -> PccProofSidecar {
    PccProofSidecar::new(
        product,
        artifact_commitment,
        sidecar.semantic_profile().to_owned(),
        sidecar.checker_profile().to_owned(),
        guarantees,
        evidence,
        assumptions,
        dependencies,
    )
    .expect("rebuilt sidecar")
}

#[test]
fn standalone_pair_checking_rejects_wrong_bytes_premises_policy_and_assumptions() {
    let (dir, psi, proof) = publish_psi_pair(MAIN);
    let sidecar = PccProofSidecar::from_bytes(&proof).expect("decode");
    let policy = receiver_policy(&sidecar);

    // The envelope's declared kind routes the pair through the bounded Psi
    // verifier: the honest pair verifies Complete, qualified by the
    // reconstructed subject, ledger and receiver admissions.
    let product = match verify_published_proof_pair(&psi, &proof, &policy) {
        PccVerificationOutcome::Complete(product) => product,
        other => panic!("expected a complete verified pair, got {other:?}"),
    };
    assert_eq!(product.product, PccProductKind::Psi);
    assert_eq!(
        product.accepted_guarantees,
        [terminal_codec::PSI_TERMINAL_VERIFIED_GUARANTEE]
    );

    // Wrong bytes: the recomputed artifact commitment is the only binding, so
    // one appended byte rejects.
    let mut tampered = psi.clone();
    tampered.push(0);
    assert!(matches!(
        verify_published_proof_pair(&tampered, &proof, &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "artifact bytes"
    ));

    // A different published Psi artifact is still the wrong bytes: pairing
    // the first sidecar with the second project's artifact rejects on the
    // content commitment, and vice versa.
    let (other_dir, other_psi, other_proof) = publish_psi_pair(DEPENDENCY_MAIN);
    assert_ne!(psi, other_psi);
    assert!(matches!(
        verify_published_proof_pair(&other_psi, &proof, &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "artifact bytes"
    ));
    let other_sidecar = PccProofSidecar::from_bytes(&other_proof).expect("decode");
    let other_policy = receiver_policy(&other_sidecar);
    assert!(matches!(
        verify_published_proof_pair(&psi, &other_proof, &other_policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "artifact bytes"
    ));
    let _ = fs::remove_dir_all(&other_dir);

    // A malformed companion rejects at the envelope before any product leg.
    let mut malformed = proof.clone();
    malformed.truncate(3);
    assert!(matches!(
        verify_published_proof_pair(&psi, &malformed, &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "sidecar envelope"
    ));

    // Wrong premises: a guarantee conditioned on a premise the receiver
    // policy does not admit rejects at claim checking.
    let mut premised = sidecar.guarantees().to_vec();
    premised[0]
        .premises
        .push("receiver-unadmitted-premise".to_owned());
    let premised = rebuild_sidecar(
        &sidecar,
        sidecar.product(),
        *sidecar.artifact_commitment(),
        premised,
        sidecar.evidence().to_vec(),
        sidecar.assumptions().to_vec(),
        sidecar.dependencies().to_vec(),
    );
    assert!(matches!(
        verify_published_proof_pair(&psi, &premised.to_bytes(), &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "guarantee premise"
    ));

    // Even when the policy admits the invented premise, the offered claim
    // still differs from the claim replay independently establishes, so the
    // reconstruction rejects it.
    let mut admitted = policy.clone();
    admitted
        .admitted_premises
        .push("receiver-unadmitted-premise".to_owned());
    assert!(matches!(
        verify_published_proof_pair(&psi, &premised.to_bytes(), &admitted),
        PccVerificationOutcome::Reject(ref r) if r.subject == "guarantees"
    ));

    // Wrong policy: a requirement the pair does not offer, and profiles the
    // receiver does not accept, each reject by name.
    let mut denied = policy.clone();
    denied.required_guarantees = vec!["omega.unsupported-claim.v1".to_owned()];
    assert!(matches!(
        verify_published_proof_pair(&psi, &proof, &denied),
        PccVerificationOutcome::Reject(ref r) if r.subject == "required guarantee"
    ));
    let mut denied = policy.clone();
    denied.accepted_semantic_profiles = vec!["other-semantics".to_owned()];
    assert!(matches!(
        verify_published_proof_pair(&psi, &proof, &denied),
        PccVerificationOutcome::Reject(ref r) if r.subject == "semantic profile"
    ));
    let mut denied = policy.clone();
    denied.accepted_checker_profiles = vec!["other-checker".to_owned()];
    assert!(matches!(
        verify_published_proof_pair(&psi, &proof, &denied),
        PccVerificationOutcome::Reject(ref r) if r.subject == "checker profile"
    ));

    // Wrong assumptions: a policy that stops admitting one closure member
    // rejects, and a sidecar that understates the closure the toolchain
    // actually relies on is caught by reconstruction even under a policy that
    // would admit the reduced set.
    let mut denied = policy.clone();
    denied.admitted_assumptions.pop();
    assert!(matches!(
        verify_published_proof_pair(&psi, &proof, &denied),
        PccVerificationOutcome::Reject(ref r) if r.subject == "assumption"
    ));
    assert!(!sidecar.assumptions().is_empty());
    let understated = rebuild_sidecar(
        &sidecar,
        sidecar.product(),
        *sidecar.artifact_commitment(),
        sidecar.guarantees().to_vec(),
        sidecar.evidence().to_vec(),
        Vec::new(),
        sidecar.dependencies().to_vec(),
    );
    let mut permissive = policy.clone();
    permissive.admitted_assumptions.clear();
    assert!(matches!(
        verify_published_proof_pair(&psi, &understated.to_bytes(), &permissive),
        PccVerificationOutcome::Reject(ref r) if r.subject == "assumption closure"
    ));

    // The bounded Psi contract carries its checkable evidence inside the
    // artifact: a companion that moves evidence into the envelope rejects.
    let evidenced = rebuild_sidecar(
        &sidecar,
        sidecar.product(),
        *sidecar.artifact_commitment(),
        sidecar.guarantees().to_vec(),
        vec![1],
        sidecar.assumptions().to_vec(),
        sidecar.dependencies().to_vec(),
    );
    assert!(matches!(
        verify_published_proof_pair(&psi, &evidenced.to_bytes(), &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "psi evidence"
    ));

    // Relabeling the same companion bytes as a native product routes to the
    // native leg, which stays fail-closed: the relabel gains nothing.
    let relabeled = rebuild_sidecar(
        &sidecar,
        PccProductKind::Native,
        *sidecar.artifact_commitment(),
        sidecar.guarantees().to_vec(),
        sidecar.evidence().to_vec(),
        sidecar.assumptions().to_vec(),
        sidecar.dependencies().to_vec(),
    );
    assert_eq!(
        verify_published_proof_pair(&psi, &relabeled.to_bytes(), &policy),
        PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
            product: PccProductKind::Native,
        })
    );

    // Exhaustion reports Incomplete through the same entry, not Reject.
    let mut exhausted = policy.clone();
    exhausted.max_artifact_bytes = 4;
    assert_eq!(
        verify_published_proof_pair(&psi, &proof, &exhausted),
        PccVerificationOutcome::Incomplete(PccIncompleteness::ArtifactBytes {
            actual: psi.len() as u64,
            limit: 4,
        })
    );

    // Standalone checking needs only the pair and the receiver policy: with
    // the source gone the same pair still verifies Complete.
    fs::remove_file(dir.join("main.omg")).expect("delete source");
    fs::remove_file(dir.join("build.omg")).expect("delete build");
    assert!(matches!(
        verify_published_proof_pair(&psi, &proof, &policy),
        PccVerificationOutcome::Complete(_)
    ));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn omitted_dependencies_require_exact_receiver_possession() {
    let (dir, psi, proof) = publish_psi_pair(DEPENDENCY_MAIN);
    let sidecar = PccProofSidecar::from_bytes(&proof).expect("decode");
    let [dependency] = sidecar.dependencies() else {
        panic!("the published pair must enumerate its omitted dependency")
    };
    assert!(
        dependency.identity.contains("Endpoint::step"),
        "unexpected dependency identity {}",
        dependency.identity
    );

    // The receiver's possessed material is reconstructed from the artifact's
    // own installation-reach declaration — the same identity and content
    // commitment an independently held copy would carry — never copied from
    // the offered claim.
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&psi).expect("decode");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("module");
    let possessed: Vec<terminal_codec::PccDependency> = module
        .root_service_reach
        .installation_dependencies
        .iter()
        .map(|reach| {
            terminal_codec::PccDependency::from_installation_reach(reach, &module.services)
                .expect("declared dependency commits")
        })
        .collect();
    assert_eq!(possessed, sidecar.dependencies().to_vec());

    let mut policy = receiver_policy(&sidecar);
    policy.possessed_dependencies = possessed;
    assert!(matches!(
        verify_published_proof_pair(&psi, &proof, &policy),
        PccVerificationOutcome::Complete(_)
    ));

    // A receiver that does not independently possess the omitted dependency
    // rejects; the name alone is not possession.
    let mut lacking = policy.clone();
    lacking.possessed_dependencies.clear();
    assert!(matches!(
        verify_published_proof_pair(&psi, &proof, &lacking),
        PccVerificationOutcome::Reject(ref r) if r.subject == "dependency"
    ));

    // Possession is by exact content commitment: a same-identity dependency
    // with different material — a nearby version or guessed compatibility —
    // rejects identically.
    let mut nearby = policy.clone();
    nearby.possessed_dependencies = vec![terminal_codec::PccDependency {
        identity: dependency.identity.clone(),
        content_commitment: [9; 32],
    }];
    assert!(matches!(
        verify_published_proof_pair(&psi, &proof, &nearby),
        PccVerificationOutcome::Reject(ref r) if r.subject == "dependency"
    ));

    // A sidecar that omits the declared dependency passes the possession
    // check vacuously but is caught by reconstruction: the producer cannot
    // shrink the inventory the artifact itself enumerates.
    let omitted = rebuild_sidecar(
        &sidecar,
        sidecar.product(),
        *sidecar.artifact_commitment(),
        sidecar.guarantees().to_vec(),
        sidecar.evidence().to_vec(),
        sidecar.assumptions().to_vec(),
        Vec::new(),
    );
    assert!(matches!(
        verify_published_proof_pair(&psi, &omitted.to_bytes(), &lacking),
        PccVerificationOutcome::Reject(ref r) if r.subject == "dependency inventory"
    ));

    // Padding the inventory with material the artifact does not declare
    // fails the same reconstruction even when the receiver possesses it.
    let mut padded_dependencies = sidecar.dependencies().to_vec();
    padded_dependencies.push(terminal_codec::PccDependency {
        identity: "test::uninstalled-material".to_owned(),
        content_commitment: [4; 32],
    });
    let padded = rebuild_sidecar(
        &sidecar,
        sidecar.product(),
        *sidecar.artifact_commitment(),
        sidecar.guarantees().to_vec(),
        sidecar.evidence().to_vec(),
        sidecar.assumptions().to_vec(),
        padded_dependencies.clone(),
    );
    let mut overpossessed = policy.clone();
    overpossessed.possessed_dependencies = padded_dependencies;
    assert!(matches!(
        verify_published_proof_pair(&psi, &padded.to_bytes(), &overpossessed),
        PccVerificationOutcome::Reject(ref r) if r.subject == "dependency inventory"
    ));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn native_pair_checking_needs_no_source_or_psi() {
    // A native receiver holds only the executable bytes, its `.proof`
    // companion, and its own pinned policy. Source and the Psi artifact are
    // producer-side material; deleting them before verification changes
    // nothing, and recomputed producer custody still cannot complete.
    let dir = write_project("    builder.pcc.psi = true;\n");
    let out = dir.join("out");
    let published = compile_native(&dir)
        .publish_retained_native_artifact(&out)
        .expect("psi pcc publication");
    let psi = read(&published.pcc_publications()[0].artifact_path);
    let executable = published
        .checked_native_executable_path()
        .expect("executable")
        .to_path_buf();
    let executable_bytes = read(&executable);

    fs::remove_file(dir.join("main.omg")).expect("delete source");
    fs::remove_file(dir.join("build.omg")).expect("delete build");
    fs::remove_file(&published.pcc_publications()[0].artifact_path).expect("delete psi companion");

    let sidecar = forged_native_sidecar(&psi, &executable_bytes);
    assert_eq!(
        verify_native_proof_sidecar(
            &executable_bytes,
            &sidecar.to_bytes(),
            &receiver_policy(&sidecar)
        ),
        PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
            product: PccProductKind::Native
        })
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn receiver_replay_never_inherits_the_producer_admission_profile() {
    // A producer publishing under a non-default admission profile discloses
    // that profile as the pair's checker profile; it does not transfer the
    // decision. The acceptance below cites a site no obligation in `MAIN`
    // ever raises, so the artifact's evidence is identical under either
    // profile and the only difference between producer and receiver is the
    // admission decision itself.
    use proof_admission::AdmissionAcceptance;
    use semantic_vocabulary::{AdmissionSiteId, EvidenceIdentity, ProfileDecisionId};

    let acceptance = AdmissionAcceptance {
        site: AdmissionSiteId::new(7).expect("site"),
        evidence_identity: EvidenceIdentity::new(8).expect("evidence identity"),
        profile_decision: ProfileDecisionId::new(9).expect("profile decision"),
    };
    let producer_profile = AdmissionProfile::from_acceptances([acceptance]);
    let dir = write_project_source(MAIN, "    builder.pcc.psi = true;\n");
    let published = compile(
        compile_request(&dir, RequestedCompileProduct::TerminalArtifact)
            .with_admission_profile(producer_profile.clone()),
    )
    .and_then(CompileOutcomes::into_single_report)
    .expect("terminal compilation")
    .publish_retained_terminal_artifact(&dir.join("out"))
    .expect("psi publication");
    let pair = &published.pcc_publications()[0];
    let (psi, proof) = (read(&pair.artifact_path), read(&pair.sidecar_path));
    let sidecar = PccProofSidecar::from_bytes(&proof).expect("decode");
    assert_eq!(
        sidecar.checker_profile(),
        terminal_codec::admission_profile_identity(&producer_profile)
    );
    assert_ne!(
        sidecar.checker_profile(),
        terminal_codec::admission_profile_identity(&AdmissionProfile::default())
    );

    // The receiver accepts the offered checker-profile name yet replays under
    // its own default profile. Replay independently establishes the default
    // profile's identity, so the offered claim no longer matches: naming the
    // producer's profile is not adopting its admissions.
    let policy = receiver_policy(&sidecar);
    assert_eq!(
        policy.accepted_checker_profiles,
        [sidecar.checker_profile().to_owned()]
    );
    assert!(matches!(
        verify_published_proof_pair(&psi, &proof, &policy),
        PccVerificationOutcome::Reject(ref r) if r.subject == "checker profile"
    ));

    // Only the receiver's own explicit admission decision completes the pair,
    // and the qualified verdict discloses exactly that acceptance.
    let mut pinned = policy.clone();
    pinned.admission_profile = producer_profile;
    let product = match verify_published_proof_pair(&psi, &proof, &pinned) {
        PccVerificationOutcome::Complete(product) => product,
        other => {
            panic!("expected the receiver's own admission decision to complete, got {other:?}")
        }
    };
    assert_eq!(product.admissions, [acceptance]);
    assert_eq!(product.checker_profile, sidecar.checker_profile());
    assert_eq!(product.policy_package_identity, "receiver-pinned-policy");

    // The same program published under the default profile verifies for the
    // default receiver: the rejection above is the producer's profile, not
    // the artifact.
    let (default_dir, default_psi, default_proof) = publish_psi_pair(MAIN);
    let default_sidecar = PccProofSidecar::from_bytes(&default_proof).expect("decode");
    assert_eq!(
        psi, default_psi,
        "the admission profile does not change the artifact bytes"
    );
    assert_ne!(
        proof, default_proof,
        "the checker profile is part of the offered claim"
    );
    assert!(matches!(
        verify_published_proof_pair(
            &default_psi,
            &default_proof,
            &receiver_policy(&default_sidecar)
        ),
        PccVerificationOutcome::Complete(_)
    ));
    let _ = fs::remove_dir_all(&default_dir);
    let _ = fs::remove_dir_all(&dir);
}
