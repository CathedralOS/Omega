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
    let inventory =
        source("omega-rust/omega/backend/images/omega-image/src/model/executable_regions.rs");
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

    let output = source("omega-rust/omega/backend/images/omega-image/src/output.rs");
    assert!(
        output.contains("pub fn evidence_report_fingerprint(self) -> u64")
            && !output.contains("pub fn evidence_fingerprint(self) -> u64"),
        "compiler-function validation's compact aggregate must remain report-only by name"
    );
    for report_field in [
        "callback_placement_identity_report_fingerprint: u64",
        "final_region_binding_report_fingerprint: u64",
        "fixed_mechanics_validation_report_fingerprint: u64",
        "fixed_mechanics_boundary_contract_report_fingerprint: u64",
        "fixed_mechanics_footprint_report_fingerprint: u64",
        "body_specification_validation_report_fingerprint: u64",
        "body_specification_boundary_contract_report_fingerprint: u64",
        "body_specification_footprint_report_fingerprint: u64",
        "composed_footprint_report_fingerprint: u64",
        "validation_report_fingerprint: u64",
        "encoded_text_report_fingerprint: u64",
        "final_compiler_text_report_fingerprint: u64",
        "relocation_envelope_report_fingerprint: u64",
        "checked_instruction_validation_report_fingerprint: u64",
        "checked_instruction_footprint_report_fingerprint: u64",
        "derivation_report_fingerprint: u64",
    ] {
        assert!(
            output.contains(report_field),
            "native output lost explicit report-only field `{report_field}`"
        );
    }
    for ambiguous_field in [
        "pub callback_placement_identity_fingerprint: u64",
        "pub final_region_binding_fingerprint: u64",
        "pub fixed_mechanics_validation_fingerprint: u64",
        "pub body_specification_validation_fingerprint: u64",
        "pub composed_footprint_fingerprint: u64",
        "pub validation_fingerprint: u64",
        "pub encoded_text_fingerprint: u64",
        "pub derivation_fingerprint: u64",
    ] {
        assert!(
            !output.contains(ambiguous_field),
            "native output recovered ambiguous compact field `{ambiguous_field}`"
        );
    }
    assert!(
        output.contains("CompilerEntryRegionBindingDigest")
            && output.contains("entry_region_evidence_digest: CompilerEntryRegionBindingDigest")
            && output.contains("footprint_digest: crate::StateFootprintEvidenceDigest")
            && output.contains("encoded_text_digest: EncodedCompilerTextDigest")
            && output.contains("relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest"),
        "report coordinates must remain beside exact or strong native-image custody"
    );

    let certificate =
        source("omega-rust/omega/backend/images/omega-image/src/footprint_certificate.rs");
    assert!(
        certificate.contains("boundary_contract_report_fingerprint: Option<u64>")
            && certificate.contains("callback_placement_identity_report_fingerprint: u64")
            && certificate.contains("binding.evidence_digest.as_bytes()")
            && certificate.contains("inventory.inventory_digest.as_bytes()"),
        "final-footprint certificate must label compact coordinates and retain strong custody"
    );

    let publication = source("omega-rust/omega/compiler/omega-compilation-report/src/lib.rs");
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
    assert!(
        publication.contains("callback_placement_identity_report_fingerprint: u64")
            && publication.contains("boundary_contract_report_fingerprint: Option<u64>")
            && !publication.contains("callback_placement_identity_fingerprint: u64")
            && !publication.contains("boundary_contract_fingerprint: Option<u64>"),
        "publication must not present callback or boundary report coordinates as authority"
    );
}
