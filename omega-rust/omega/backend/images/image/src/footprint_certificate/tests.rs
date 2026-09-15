//! Footprint certificate tests.

use super::{
    CompilerEntryFootprintBindingEvidence, CompilerFunctionValidationEvidence,
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest,
    CompilerTextValidationEvidence, EncodedCompilerTextDigest, FinalCompilerTextDigest,
    FinalFootprintCertificate, FinalFootprintClass, FinalFootprintCoverage,
    PlacedExecutableRegionInventory,
};
use target::NativeTarget;

fn empty_inventory() -> PlacedExecutableRegionInventory {
    let image = crate::FinalImage::with_capacity(
        NativeTarget::host(),
        crate::FinalImageMemory::default(),
        Default::default(),
        0,
        0,
        0,
    );
    crate::place_executable_regions(
        &image,
        crate::FinalImageLayout {
            text_address: 0x1000,
            ..crate::FinalImageLayout::default()
        },
    )
    .expect("empty executable inventory should place")
}

fn entry_footprint_binding(
    inventory: &PlacedExecutableRegionInventory,
) -> CompilerEntryFootprintBindingEvidence {
    let mut binding = CompilerEntryFootprintBindingEvidence {
        entry_region_evidence_digest: crate::CompilerEntryRegionBindingDigest::from_digest(
            [20; 32],
        ),
        entry_region_evidence_report_fingerprint: 20,
        final_region_binding_report_fingerprint: 19,
        prior_inventory_digest: crate::PlacedExecutableRegionInventoryDigest::from_digest([12; 32]),
        prior_inventory_report_fingerprint: 12,
        footprint_digest: crate::StateFootprintEvidenceDigest::from_digest([2; 32]),
        footprint_report_fingerprint: 2,
        resulting_inventory_digest: inventory.inventory_digest,
        resulting_inventory_report_fingerprint: inventory.inventory_report_fingerprint,
        evidence_digest: crate::CompilerEntryFootprintBindingDigest::from_digest([0; 32]),
        evidence_report_fingerprint: 0,
    };
    binding.evidence_digest = binding.recomputed_evidence_digest();
    binding.evidence_report_fingerprint = binding.recomputed_evidence_report_fingerprint();
    binding
}

fn compiler_text_validation() -> CompilerTextValidationEvidence {
    let mut evidence = CompilerTextValidationEvidence {
        encoded_text_digest: EncodedCompilerTextDigest::from_digest([1; 32]),
        final_compiler_text_digest: FinalCompilerTextDigest::from_digest([2; 32]),
        relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest::from_digest([3; 32]),
        derivation_digest: CompilerTextDerivationDigest::from_digest([0; 32]),
        encoded_text_report_fingerprint: 4,
        final_compiler_text_report_fingerprint: 5,
        relocation_envelope_report_fingerprint: 6,
        checked_instruction_validation_report_fingerprint: 7,
        checked_instruction_footprint_report_fingerprint: 18,
        derivation_report_fingerprint: 8,
        text_relocation_count: 9,
        checked_instruction_validation_count: 10,
    };
    evidence.derivation_digest = evidence.recomputed_derivation_digest();
    evidence
}

fn empty_compiler_text_validation() -> CompilerTextValidationEvidence {
    let mut evidence = CompilerTextValidationEvidence {
        encoded_text_digest: EncodedCompilerTextDigest::from_digest([0; 32]),
        final_compiler_text_digest: FinalCompilerTextDigest::from_digest([0; 32]),
        relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest::from_digest([0; 32]),
        derivation_digest: CompilerTextDerivationDigest::from_digest([0; 32]),
        encoded_text_report_fingerprint: 0,
        final_compiler_text_report_fingerprint: 0,
        relocation_envelope_report_fingerprint: 0,
        checked_instruction_validation_report_fingerprint: 0,
        checked_instruction_footprint_report_fingerprint: 0,
        derivation_report_fingerprint: 0,
        text_relocation_count: 0,
        checked_instruction_validation_count: 0,
    };
    evidence.derivation_digest = evidence.recomputed_derivation_digest();
    evidence
}

fn certificate() -> FinalFootprintCertificate {
    let inventory = empty_inventory();
    let binding = entry_footprint_binding(&inventory);
    FinalFootprintCertificate::current(
        Some(1),
        2,
        3,
        21,
        compiler_text_validation(),
        CompilerFunctionValidationEvidence {
            function_count: 1,
            instruction_count: 2,
            zero_width_instruction_count: 0,
            frame_prologue_byte_count: 4,
            frame_epilogue_byte_count: 8,
            fragment_manifest_report_fingerprint: 14,
            frame_application_report_fingerprint: 15,
            boundary_contract_report_fingerprint: Some(1),
            final_region_binding_report_fingerprint: 19,
            final_text_validation_report_fingerprint: 8,
        },
        Some(binding),
        inventory,
    )
    .expect("complete certificate")
}

#[test]
fn complete_certificate_binds_coverage_placement_text_and_inventory() {
    let certificate = certificate();
    certificate.validate_identity().expect("valid identity");

    for drifted in [
        {
            let mut value = certificate.clone();
            value.marker = "omega.final-footprint-certificate.stale";
            value
        },
        {
            let mut value = certificate.clone();
            value
                .coverage
                .missing_classes
                .push(FinalFootprintClass::CompilerFunctions);
            value
        },
        {
            let mut value = certificate.clone();
            value.boundary_contract_report_fingerprint = Some(99);
            value
        },
        {
            let mut value = certificate.clone();
            value.callback_placement_identity_report_fingerprint = 99;
            value
        },
        {
            let mut value = certificate.clone();
            value.compiler_text_validation.derivation_report_fingerprint = 99;
            value
        },
        {
            let mut value = certificate.clone();
            value.compiler_text_validation.encoded_text_digest =
                EncodedCompilerTextDigest::from_digest([99; 32]);
            value
        },
        {
            let mut value = certificate.clone();
            value
                .compiler_function_validation
                .final_text_validation_report_fingerprint = 99;
            value
        },
        {
            let mut value = certificate.clone();
            value.compiler_function_validation.instruction_count = 99;
            value
        },
        {
            let mut value = certificate.clone();
            value.compiler_function_validation.frame_prologue_byte_count = 99;
            value
        },
        {
            let mut value = certificate.clone();
            value.compiler_function_validation.frame_epilogue_byte_count = 99;
            value
        },
        {
            let mut value = certificate.clone();
            value
                .compiler_function_validation
                .boundary_contract_report_fingerprint = None;
            value
        },
        {
            let mut value = certificate.clone();
            value
                .compiler_function_validation
                .boundary_contract_report_fingerprint = Some(99);
            value
        },
        {
            let mut value = certificate.clone();
            value
                .compiler_function_validation
                .final_region_binding_report_fingerprint = 99;
            value
        },
        {
            let mut value = certificate.clone();
            value
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .prior_inventory_report_fingerprint = 99;
            value
        },
        {
            let mut value = certificate.clone();
            value.compiler_entry_footprint_binding = None;
            value
        },
        {
            let mut value = certificate.clone();
            value.inventory.inventory_report_fingerprint = 99;
            value
        },
        {
            let mut value = certificate.clone();
            value.inventory.inventory_digest =
                crate::PlacedExecutableRegionInventoryDigest::from_digest([99; 32]);
            value
        },
    ] {
        assert!(drifted.validate_identity().is_err());
    }
}

#[test]
fn optional_function_boundary_preserves_exact_entry_footprint_requirements() {
    let original = certificate();
    for (boundary, reported_boundary, retain_binding, accepted) in [
        (Some(1), Some(1), true, true),
        (Some(1), None, true, true),
        (None, None, false, true),
        (Some(1), Some(99), true, false),
        (Some(1), Some(1), false, false),
        (Some(1), None, false, false),
        (None, Some(1), false, false),
        (None, None, true, false),
    ] {
        let mut function_validation = original.compiler_function_validation;
        function_validation.boundary_contract_report_fingerprint = reported_boundary;
        let binding = if retain_binding {
            original.compiler_entry_footprint_binding
        } else {
            None
        };
        let result = FinalFootprintCertificate::current(
            boundary,
            original.implementation_evidence_report_fingerprint,
            original.implementation_fragment_count,
            original.callback_placement_identity_report_fingerprint,
            original.compiler_text_validation,
            function_validation,
            binding,
            original.inventory.clone(),
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "boundary {boundary:?}, report {reported_boundary:?}, binding {retain_binding}"
        );
        if let Ok(certificate) = result {
            certificate
                .validate_identity()
                .expect("accepted boundary replays");
        }
    }
}

#[test]
fn compact_certificate_identity_cannot_substitute_strong_native_evidence() {
    let certificate = certificate();
    let mut substituted = certificate.clone();
    substituted.inventory.inventory_digest =
        crate::PlacedExecutableRegionInventoryDigest::from_digest([99; 32]);

    assert_eq!(
        substituted.inventory.inventory_report_fingerprint,
        certificate.inventory.inventory_report_fingerprint
    );
    assert_eq!(
        substituted.certificate_report_fingerprint,
        certificate.certificate_report_fingerprint
    );
    assert!(substituted.validate_identity().is_err());

    let mut substituted = certificate.clone();
    let binding = substituted
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding");
    binding.footprint_digest = crate::StateFootprintEvidenceDigest::from_digest([99; 32]);
    binding.evidence_digest = binding.recomputed_evidence_digest();
    assert_eq!(
        substituted.certificate_report_fingerprint,
        certificate.certificate_report_fingerprint
    );
    assert!(substituted.validate_identity().is_err());

    let mut substituted = certificate.clone();
    let binding = substituted
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding");
    binding.entry_region_evidence_digest =
        crate::CompilerEntryRegionBindingDigest::from_digest([98; 32]);
    assert_eq!(
        binding.entry_region_evidence_report_fingerprint,
        certificate
            .compiler_entry_footprint_binding
            .expect("original entry binding")
            .entry_region_evidence_report_fingerprint
    );
    assert_eq!(
        substituted.certificate_report_fingerprint,
        certificate.certificate_report_fingerprint
    );
    assert!(substituted.validate_identity().is_err());
}

#[test]
fn complete_coverage_classifies_admitted_leaves_as_absent() {
    let coverage = FinalFootprintCoverage::current();
    assert!(
        coverage
            .absent_by_construction_classes
            .contains(&FinalFootprintClass::AdmittedLeaves)
    );
    assert!(
        !coverage
            .missing_classes
            .contains(&FinalFootprintClass::AdmittedLeaves)
    );
    coverage.validate_normalized().expect("normalized coverage");
}

#[test]
fn coverage_rejects_conflicting_class_statuses() {
    let mut coverage = FinalFootprintCoverage::current();
    coverage
        .missing_classes
        .push(FinalFootprintClass::AdmittedLeaves);
    assert!(
        coverage
            .validate_normalized()
            .expect_err("one class cannot be both absent and missing")
            .message
            .contains("both absent-by-construction and missing")
    );
}

#[test]
fn region_complete_certificate_rejects_gaps() {
    let mut inventory = certificate().inventory;
    inventory
        .unclassified_gaps
        .push(crate::PlacedExecutableGap {
            section_offset: 0,
            address: 0x1000,
            byte_count: 1,
            byte_digest: crate::PlacedExecutableGapBytesDigest::from_digest([1; 32]),
            byte_report_fingerprint: 1,
        });
    assert!(
        FinalFootprintCertificate::current(
            None,
            0,
            0,
            0,
            empty_compiler_text_validation(),
            CompilerFunctionValidationEvidence {
                function_count: 0,
                instruction_count: 0,
                zero_width_instruction_count: 0,
                frame_prologue_byte_count: 0,
                frame_epilogue_byte_count: 0,
                fragment_manifest_report_fingerprint: 0,
                frame_application_report_fingerprint: 0,
                boundary_contract_report_fingerprint: None,
                final_region_binding_report_fingerprint: 0,
                final_text_validation_report_fingerprint: 0,
            },
            None,
            inventory,
        )
        .is_err()
    );
}
