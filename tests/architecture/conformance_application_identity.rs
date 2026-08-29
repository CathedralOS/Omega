//! Guard the authority-bearing closed-conformance application join.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

fn source(relative: &str) -> String {
    fs::read_to_string(workspace_root().join(relative))
        .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

#[test]
fn closed_conformance_dispatch_never_authorizes_with_a_compact_identity_alone() {
    let typed = source("source/omega-rust/psi/representations/psi-typed-trees/src/typed_trees.rs");
    let checked =
        source("source/omega-rust/psi/representations/psi-checked-trees/src/proof/contracts.rs");
    let terminal = source("source/omega-rust/psi/representations/psi-terminal/src/module.rs");
    let verifier =
        source("source/omega-rust/psi/semantics/psi-terminal-verifier/src/validation/evidence.rs");

    for (owner, carrier) in [
        ("typed dispatch", &typed),
        ("checked dispatch", &checked),
        ("Terminal application/dispatch", &terminal),
    ] {
        assert!(
            carrier.contains("application_commitment")
                || carrier.contains("conformance_application_commitment"),
            "{owner} lost the strong closed-conformance application carrier"
        );
    }
    assert!(
        terminal.contains("omega.psi.terminal.closed-conformance-application.v1\\0")
            && terminal.contains("Sha256"),
        "Terminal closed-conformance identity lost its domain-separated SHA-256 owner"
    );
    assert!(
        verifier.contains("application.commitment == dispatch.conformance_application_commitment"),
        "Terminal proof-output dispatch no longer rejoins the strong application commitment"
    );
    assert!(
        typed.contains("pub report_fingerprint: u64")
            && checked.contains("pub application_report_fingerprint: u64")
            && terminal.contains("pub report_fingerprint: u64")
            && terminal.contains("pub conformance_application_report_fingerprint: u64")
            && !typed.contains("pub application_fingerprint: u64")
            && !checked.contains("pub application_fingerprint: u64")
            && !terminal.contains("pub conformance_application_fingerprint: u64"),
        "closed-conformance compact coordinates must remain explicitly report-only"
    );
}
