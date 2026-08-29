//! Guard native-image inventory authority against compact-only regressions.

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
fn native_inventory_compact_coordinates_are_report_only_and_publication_keeps_the_digest() {
    let inventory = source(
        "source/omega-rust/omega/backend/images/omega-image/src/model/executable_regions.rs",
    );
    for report_field in [
        "byte_report_fingerprint: u64",
        "text_report_fingerprint: u64",
        "inventory_report_fingerprint: u64",
    ] {
        assert!(
            inventory.contains(report_field),
            "native inventory lost explicit report-only field `{report_field}`"
        );
    }
    for ambiguous_field in [
        "pub byte_fingerprint: u64",
        "pub text_fingerprint: u64",
        "pub inventory_fingerprint: u64",
    ] {
        assert!(
            !inventory.contains(ambiguous_field),
            "native inventory recovered ambiguous compact field `{ambiguous_field}`"
        );
    }
    assert!(
        inventory.contains("omega.placed-executable-region-inventory.sha256.v1\\0")
            && inventory.contains("PlacedExecutableRegionInventoryDigest"),
        "native inventory must retain its domain-separated strong identity"
    );

    let output = source("source/omega-rust/omega/backend/images/omega-image/src/output.rs");
    assert!(
        output.contains("pub fn evidence_report_fingerprint(self) -> u64")
            && !output.contains("pub fn evidence_fingerprint(self) -> u64"),
        "compiler-function validation's compact aggregate must remain report-only by name"
    );

    let publication =
        source("source/omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
    assert!(
        publication
            .contains("inventory_digest: omega_image::PlacedExecutableRegionInventoryDigest",)
            && publication.contains("digest.update(inventory_digest.as_bytes());")
            && publication.contains("flat.inventory_digest == bundle.inventory_digest"),
        "native publication and bundle replay must retain the strong image-inventory digest"
    );
    assert!(
        publication.contains("inventory_report_fingerprint: u64")
            && !publication.contains("inventory_fingerprint: u64"),
        "publication's compact inventory coordinate must remain explicitly report-only"
    );
}
