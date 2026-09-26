//! Footprint certificate tests.

use super::{
    CompilerEntryFootprintBindingEvidence, CompilerFunctionValidationEvidence,
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest,
    CompilerTextValidationEvidence, EncodedCompilerTextDigest, FinalCompilerTextDigest,
    FinalFootprintCertificate, FinalFootprintCertificateDigest, FinalFootprintClass,
    FinalFootprintCoverage, FinalFootprintCoverageDigest, FinalFootprintPlacementBindingDigest,
    PlacedExecutableRegionInventory, certificate_digest, certificate_report_fingerprint,
    placement_binding_digest, placement_binding_report_fingerprint,
};
use calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use target::NativeTarget;

/// A two-region inventory over non-uniform text with no unclassified gap:
/// `entry` (a compiler function carrying a footprint) at `[0..4)` and
/// `host_call` (an import thunk without one) at `[4..8)`, so the certificate
/// retains real region rows whose substitutions the matrix can exercise.
fn placed_inventory() -> (crate::FinalImage, PlacedExecutableRegionInventory) {
    let mut image = crate::FinalImage::with_capacity(
        NativeTarget::host(),
        crate::FinalImageMemory {
            text: (0usize..8).map(|index| (index * 7 + 3) as u8).collect(),
            ..crate::FinalImageMemory::default()
        },
        Default::default(),
        0,
        0,
        0,
    );
    image.executable_regions.extend([
        crate::FinalExecutableRegion {
            origin: crate::FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 0,
            byte_count: 4,
            symbol: "entry".into(),
            footprint: Some(StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            )),
        },
        crate::FinalExecutableRegion {
            origin: crate::FinalExecutableRegionOrigin::ImportThunk,
            section_offset: 4,
            byte_count: 4,
            symbol: "host_call".into(),
            footprint: None,
        },
    ]);
    let inventory = crate::place_executable_regions(
        &image,
        crate::FinalImageLayout {
            text_address: 0x1000,
            ..crate::FinalImageLayout::default()
        },
    )
    .expect("the fixture regions place");
    (image, inventory)
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
    let (_, inventory) = placed_inventory();
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

/// A certificate retaining no entry-footprint custody: no boundary contract
/// coordinate and no binding evidence.
fn certificate_without_entry_binding() -> FinalFootprintCertificate {
    let (_, inventory) = placed_inventory();
    FinalFootprintCertificate::current(
        None,
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
            boundary_contract_report_fingerprint: None,
            final_region_binding_report_fingerprint: 19,
            final_text_validation_report_fingerprint: 8,
        },
        None,
        inventory,
    )
    .expect("certificate without entry custody")
}

/// Honestly reseal the nested evidence seals the certificate retains: the
/// text derivation digest, the entry-footprint binding's own evidence seals,
/// and the placed inventory identity.
fn reseal_nested(certificate: &mut FinalFootprintCertificate) {
    certificate.compiler_text_validation.derivation_digest = certificate
        .compiler_text_validation
        .recomputed_derivation_digest();
    if let Some(binding) = certificate.compiler_entry_footprint_binding.as_mut() {
        binding.evidence_report_fingerprint = binding.recomputed_evidence_report_fingerprint();
        binding.evidence_digest = binding.recomputed_evidence_digest();
    }
    certificate.inventory.inventory_report_fingerprint =
        crate::final_image::executable_inventory_report_fingerprint(
            certificate.inventory.text_address,
            certificate.inventory.text_byte_count,
            certificate.inventory.text_report_fingerprint,
            &certificate.inventory.regions,
            &certificate.inventory.unclassified_gaps,
        );
    certificate.inventory.inventory_digest = crate::final_image::executable_inventory_digest(
        certificate.inventory.text_address,
        certificate.inventory.text_byte_count,
        certificate.inventory.text_digest,
        &certificate.inventory.regions,
        &certificate.inventory.unclassified_gaps,
    );
}

/// Honestly recompute the certificate-level seals — coverage identity,
/// placement binding, and the certificate identity itself — from the
/// record's current nested evidence. Nested seals are left untouched, so a
/// stale nested seal stays stale and fails at its own check rather than at a
/// containing comparison the mutation never reached.
fn reseal_certificate(certificate: &mut FinalFootprintCertificate) {
    certificate.coverage_report_fingerprint = certificate.coverage.report_fingerprint();
    certificate.coverage_digest = certificate.coverage.digest();
    certificate.boundary_placement_binding_digest = placement_binding_digest(
        certificate.boundary_contract_report_fingerprint,
        certificate.implementation_evidence_report_fingerprint,
        certificate.implementation_fragment_count,
        certificate.callback_placement_identity_report_fingerprint,
        &certificate.compiler_text_validation,
        certificate.compiler_function_validation,
        certificate.compiler_entry_footprint_binding,
        &certificate.inventory,
    );
    certificate.boundary_placement_binding_report_fingerprint =
        placement_binding_report_fingerprint(
            certificate.boundary_contract_report_fingerprint,
            certificate.implementation_evidence_report_fingerprint,
            certificate.callback_placement_identity_report_fingerprint,
            certificate
                .compiler_text_validation
                .derivation_report_fingerprint,
            certificate
                .compiler_function_validation
                .evidence_report_fingerprint(),
            certificate
                .compiler_entry_footprint_binding
                .map(|binding| binding.evidence_report_fingerprint)
                .unwrap_or_default(),
            certificate.inventory.inventory_report_fingerprint,
        );
    certificate.certificate_report_fingerprint = certificate_report_fingerprint(
        certificate.coverage_report_fingerprint,
        certificate.boundary_placement_binding_report_fingerprint,
        certificate
            .compiler_text_validation
            .derivation_report_fingerprint,
        certificate
            .compiler_function_validation
            .evidence_report_fingerprint(),
        certificate
            .compiler_entry_footprint_binding
            .map(|binding| binding.evidence_report_fingerprint)
            .unwrap_or_default(),
        certificate.inventory.inventory_report_fingerprint,
    );
    certificate.certificate_digest = certificate_digest(
        certificate.coverage_digest,
        certificate.boundary_placement_binding_digest,
        &certificate.compiler_text_validation,
        certificate.compiler_function_validation,
        certificate.compiler_entry_footprint_binding,
        &certificate.inventory,
    );
}

/// Honestly reseal every seal after a record-level substitution, so a replay
/// rejection pins the mutated field rather than any stale seal above it.
fn reseal(certificate: &mut FinalFootprintCertificate) {
    reseal_nested(certificate);
    reseal_certificate(certificate);
}

/// Every retained field of [`FinalFootprintCertificate`] substitutes
/// independently and rejects at `validate_identity` replay under an honestly
/// recomputed containing identity: the marker, each of the certificate's own
/// seals, the boundary-contract and implementation-evidence joins into the
/// nested function validation and entry-footprint binding, every join and
/// well-formedness field of the binding itself, and every inventory axis
/// through the inventory digest seal plus the resulting-inventory custody
/// join. Coverage flag and roster substitutions that cannot keep the
/// normalized disjoint shape reject at the record's normalization admission.
/// Fields the replay cannot re-derive — compact report coordinates inside
/// the nested evidence and `implementation_fragment_count` — stay canonical
/// at this join and remain bound by the published certificate identity; see
/// `final_footprint_certificate_compact_fields_stay_identity_bound`.
#[test]
fn final_footprint_certificate_rejects_every_one_field_substitution() {
    let authentic = certificate();
    authentic
        .validate_identity()
        .expect("the authentic certificate replays");
    assert_eq!(authentic.inventory.regions.len(), 2);

    let rejects_at_replay = |name: &'static str, mutated: &FinalFootprintCertificate| {
        assert_ne!(mutated, &authentic, "{name} must change the record");
        assert!(
            mutated.validate_identity().is_err(),
            "{name} must reject at independent replay"
        );
    };

    // --- the marker is compared verbatim; the seals hash the constant ---
    let mut mutated = authentic.clone();
    mutated.marker = "omega.final-footprint-certificate.stale";
    reseal(&mut mutated);
    rejects_at_replay("a substituted certificate marker", &mutated);

    // --- the certificate's own seals: a stale seal is the substitution ---
    let mut mutated = authentic.clone();
    mutated.certificate_digest = FinalFootprintCertificateDigest::from_digest([0xee; 32]);
    rejects_at_replay("a stale certificate digest", &mutated);

    let mut mutated = authentic.clone();
    mutated.certificate_report_fingerprint ^= 1;
    rejects_at_replay("a stale certificate report fingerprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.coverage_digest = FinalFootprintCoverageDigest::from_digest([0xee; 32]);
    rejects_at_replay("a stale coverage digest", &mutated);

    let mut mutated = authentic.clone();
    mutated.coverage_report_fingerprint ^= 1;
    rejects_at_replay("a stale coverage report fingerprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.boundary_placement_binding_digest =
        FinalFootprintPlacementBindingDigest::from_digest([0xee; 32]);
    rejects_at_replay("a stale placement binding digest", &mutated);

    let mut mutated = authentic.clone();
    mutated.boundary_placement_binding_report_fingerprint ^= 1;
    rejects_at_replay("a stale placement binding report fingerprint", &mutated);

    // --- nested seals: honestly reseal only the certificate level so the
    // rejection lands on the nested seal check or custody join ---
    let mut mutated = authentic.clone();
    mutated.compiler_text_validation.derivation_digest =
        CompilerTextDerivationDigest::from_digest([0xee; 32]);
    reseal_certificate(&mut mutated);
    rejects_at_replay("a stale text derivation digest", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .evidence_digest = crate::CompilerEntryFootprintBindingDigest::from_digest([0xee; 32]);
    reseal_certificate(&mut mutated);
    rejects_at_replay("a stale entry-binding evidence digest", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .evidence_report_fingerprint ^= 1;
    reseal_certificate(&mut mutated);
    rejects_at_replay("a stale entry-binding evidence fingerprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.inventory_digest =
        crate::PlacedExecutableRegionInventoryDigest::from_digest([0xee; 32]);
    reseal_certificate(&mut mutated);
    rejects_at_replay("a stale inventory digest", &mutated);

    // The compact inventory fingerprint is not in the digest seal; the
    // resulting-inventory custody join still binds it.
    let mut mutated = authentic.clone();
    mutated.inventory.inventory_report_fingerprint ^= 1;
    reseal_certificate(&mut mutated);
    rejects_at_replay("a stale inventory report fingerprint", &mutated);

    // --- certificate scalar coordinates joined into nested evidence ---
    let mut mutated = authentic.clone();
    mutated.boundary_contract_report_fingerprint = Some(99);
    reseal(&mut mutated);
    rejects_at_replay("a substituted boundary contract coordinate", &mutated);

    let mut mutated = authentic.clone();
    mutated.boundary_contract_report_fingerprint = None;
    reseal(&mut mutated);
    rejects_at_replay("a dropped boundary contract coordinate", &mutated);

    let mut mutated = authentic.clone();
    mutated.implementation_evidence_report_fingerprint = 99;
    reseal(&mut mutated);
    rejects_at_replay("a substituted implementation evidence coordinate", &mutated);

    // --- coverage flags: a single-flag substitution cannot keep the
    // completeness invariant, so each rejects at normalization admission ---
    let mut mutated = authentic.clone();
    mutated.coverage.enumeration_complete = false;
    reseal(&mut mutated);
    rejects_at_replay("a dropped enumeration completeness claim", &mutated);

    let mut mutated = authentic.clone();
    mutated.coverage.region_enumeration_complete = false;
    reseal(&mut mutated);
    rejects_at_replay("a dropped region enumeration claim", &mutated);

    let mut mutated = authentic.clone();
    mutated.coverage.footprint_enumeration_complete = false;
    reseal(&mut mutated);
    rejects_at_replay("a dropped footprint enumeration claim", &mutated);

    // --- coverage rosters: order, duplication, and disjointness are the
    // normalized shape; violations reject at normalization admission ---
    let mut mutated = authentic.clone();
    mutated.coverage.covered_classes.swap(0, 1);
    reseal(&mut mutated);
    rejects_at_replay("a reordered covered roster", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .coverage
        .covered_classes
        .insert(1, FinalFootprintClass::CompilerFunctions);
    reseal(&mut mutated);
    rejects_at_replay("a duplicated covered class", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .coverage
        .covered_classes
        .push(FinalFootprintClass::AdmittedLeaves);
    reseal(&mut mutated);
    rejects_at_replay("a covered class that is absent by construction", &mutated);

    let mut mutated = authentic.clone();
    mutated.coverage.absent_by_construction_classes.swap(0, 1);
    reseal(&mut mutated);
    rejects_at_replay("a reordered absent-by-construction roster", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .coverage
        .absent_by_construction_classes
        .insert(0, FinalFootprintClass::CompilerFunctions);
    reseal(&mut mutated);
    rejects_at_replay("an absent-by-construction class that is covered", &mutated);

    let mut mutated = authentic.clone();
    mutated.coverage.final_byte_validated_classes.swap(0, 1);
    reseal(&mut mutated);
    rejects_at_replay("a reordered final-byte-validated roster", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .coverage
        .final_byte_validated_classes
        .insert(0, FinalFootprintClass::CatalogCheckedAssembly);
    reseal(&mut mutated);
    rejects_at_replay("a duplicated final-byte-validated class", &mutated);

    // A missing class that conflicts with no other roster still rejects: a
    // complete footprint enumeration cannot retain missing classes.
    let mut mutated = authentic.clone();
    mutated
        .coverage
        .missing_classes
        .push(FinalFootprintClass::CatalogCheckedAssembly);
    reseal(&mut mutated);
    rejects_at_replay("a missing class under a complete enumeration", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .coverage
        .missing_classes
        .push(FinalFootprintClass::CompilerFunctions);
    reseal(&mut mutated);
    rejects_at_replay("a missing class that is also covered", &mutated);

    // --- nested compiler-function validation joins ---
    let mut mutated = authentic.clone();
    mutated
        .compiler_function_validation
        .boundary_contract_report_fingerprint = Some(99);
    reseal(&mut mutated);
    rejects_at_replay(
        "a function report naming a different boundary contract",
        &mutated,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_function_validation
        .final_region_binding_report_fingerprint = 99;
    reseal(&mut mutated);
    rejects_at_replay(
        "a function report naming a different region binding",
        &mutated,
    );

    // --- entry-footprint binding presence and every join coordinate ---
    let mut mutated = authentic.clone();
    mutated.compiler_entry_footprint_binding = None;
    reseal(&mut mutated);
    rejects_at_replay("a dropped entry-footprint binding", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .final_region_binding_report_fingerprint = 99;
    reseal(&mut mutated);
    rejects_at_replay("a binding naming a different region binding", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .final_region_binding_report_fingerprint = 0;
    reseal(&mut mutated);
    rejects_at_replay("a binding with a zero region binding coordinate", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .footprint_report_fingerprint = 99;
    reseal(&mut mutated);
    rejects_at_replay(
        "a binding naming different implementation evidence",
        &mutated,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .footprint_report_fingerprint = 0;
    reseal(&mut mutated);
    rejects_at_replay("a binding with a zero footprint coordinate", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .resulting_inventory_digest =
        crate::PlacedExecutableRegionInventoryDigest::from_digest([0xee; 32]);
    reseal(&mut mutated);
    rejects_at_replay("a binding naming a different resulting inventory", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .resulting_inventory_report_fingerprint ^= 1;
    reseal(&mut mutated);
    rejects_at_replay(
        "a binding naming a different resulting inventory fingerprint",
        &mutated,
    );

    // The binding is the sole authorized mutation record: a prior identity
    // equal to the resulting one claims the footprint attachment changed
    // nothing.
    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .prior_inventory_digest = authentic
        .compiler_entry_footprint_binding
        .expect("entry binding")
        .resulting_inventory_digest;
    reseal(&mut mutated);
    rejects_at_replay(
        "a binding whose prior inventory equals its result",
        &mutated,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .prior_inventory_report_fingerprint = authentic
        .compiler_entry_footprint_binding
        .expect("entry binding")
        .resulting_inventory_report_fingerprint;
    reseal(&mut mutated);
    rejects_at_replay(
        "a binding whose prior inventory fingerprint equals its result",
        &mutated,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .prior_inventory_report_fingerprint = 0;
    reseal(&mut mutated);
    rejects_at_replay("a binding with a zero prior inventory coordinate", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .entry_region_evidence_report_fingerprint = 0;
    reseal(&mut mutated);
    rejects_at_replay("a binding with a zero entry-region coordinate", &mutated);

    // --- the retained inventory: every content substitution diverges the
    // inventory identity the entry-footprint custody join binds; the
    // byte-level partition replay remains the inventory's own lane ---
    let mut mutated = authentic.clone();
    mutated.inventory.text_address += 0x1000;
    reseal(&mut mutated);
    rejects_at_replay("a substituted inventory text base", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.text_byte_count += 4;
    reseal(&mut mutated);
    rejects_at_replay("an extended inventory text extent", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.text_digest = crate::FinalExecutableTextDigest::from_digest([0xee; 32]);
    reseal(&mut mutated);
    rejects_at_replay("a foreign inventory text digest", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.text_report_fingerprint ^= 1;
    reseal(&mut mutated);
    rejects_at_replay("a substituted inventory text fingerprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions[0].section_offset = 1;
    reseal(&mut mutated);
    rejects_at_replay("a shifted region section offset", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions[0].address += 1;
    reseal(&mut mutated);
    rejects_at_replay("a substituted region address", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions[0].byte_count = 8;
    reseal(&mut mutated);
    rejects_at_replay("an extended region byte count", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions[0].byte_digest =
        crate::PlacedExecutableRegionBytesDigest::from_digest([0xee; 32]);
    reseal(&mut mutated);
    rejects_at_replay("a foreign region byte digest", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions[0].byte_report_fingerprint ^= 1;
    reseal(&mut mutated);
    rejects_at_replay("a substituted region byte fingerprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions[0].symbol = "forged".into();
    reseal(&mut mutated);
    rejects_at_replay("a substituted region symbol", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions[0].origin = crate::FinalExecutableRegionOrigin::ImportThunk;
    reseal(&mut mutated);
    rejects_at_replay("a remarked region origin", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions[0].footprint = None;
    reseal(&mut mutated);
    rejects_at_replay("a dropped region footprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions[1].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rcx]),
        MachineStateSet::new([MachineState::SegmentState]),
    ));
    reseal(&mut mutated);
    rejects_at_replay("an added region footprint", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions.remove(0);
    reseal(&mut mutated);
    rejects_at_replay("a dropped region row", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions.swap(0, 1);
    reseal(&mut mutated);
    rejects_at_replay("a reordered region roster", &mutated);

    let mut mutated = authentic.clone();
    mutated
        .inventory
        .regions
        .push(mutated.inventory.regions[0].clone());
    reseal(&mut mutated);
    rejects_at_replay("a duplicated region row", &mutated);

    let mut mutated = authentic.clone();
    mutated.inventory.regions.insert(
        1,
        crate::PlacedExecutableRegion {
            origin: crate::FinalExecutableRegionOrigin::ImportThunk,
            section_offset: 4,
            address: 0x1004,
            byte_count: 4,
            byte_digest: crate::PlacedExecutableRegionBytesDigest::from_digest([0xee; 32]),
            byte_report_fingerprint: 0xee,
            symbol: "forged".into(),
            footprint: None,
        },
    );
    reseal(&mut mutated);
    rejects_at_replay("a foreign region row", &mutated);

    // A region-complete certificate cannot retain an executable gap; gap row
    // fields are not representable inside one and are covered by the
    // inventory's own byte-level substitution matrix.
    let mut mutated = authentic.clone();
    mutated
        .inventory
        .unclassified_gaps
        .push(crate::PlacedExecutableGap {
            section_offset: 8,
            address: 0x1008,
            byte_count: 1,
            byte_digest: crate::PlacedExecutableGapBytesDigest::from_digest([1; 32]),
            byte_report_fingerprint: 1,
        });
    reseal(&mut mutated);
    rejects_at_replay("a retained executable gap", &mutated);

    // A whole-inventory substitution: the foreign inventory is honestly
    // sealed by placement yet is not the inventory the binding binds.
    let mut mutated = authentic.clone();
    mutated.inventory = {
        let mut foreign_image = crate::FinalImage::with_capacity(
            NativeTarget::host(),
            crate::FinalImageMemory {
                text: vec![0x5a; 4],
                ..crate::FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        foreign_image
            .executable_regions
            .push(crate::FinalExecutableRegion {
                origin: crate::FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 4,
                symbol: "entry".into(),
                footprint: None,
            });
        crate::place_executable_regions(&foreign_image, crate::FinalImageLayout::default())
            .expect("foreign inventory places")
    };
    reseal(&mut mutated);
    rejects_at_replay("a foreign inventory", &mutated);

    // --- a certificate retaining no entry-footprint custody cannot gain the
    // boundary coordinate or the binding alone ---
    let binding_free = certificate_without_entry_binding();
    binding_free
        .validate_identity()
        .expect("the binding-free certificate replays");

    let mut mutated = binding_free.clone();
    mutated.boundary_contract_report_fingerprint = Some(1);
    reseal(&mut mutated);
    assert!(
        mutated.validate_identity().is_err(),
        "a boundary contract without binding custody must reject"
    );

    let mut mutated = binding_free.clone();
    mutated.compiler_entry_footprint_binding = Some(entry_footprint_binding(&mutated.inventory));
    reseal(&mut mutated);
    assert!(
        mutated.validate_identity().is_err(),
        "entry-footprint custody without a boundary contract must reject"
    );
}

/// `validate_identity` is the certificate's complete self-contained replay;
/// the lane's only consumer was deleted with the StateGraph route in
/// `f6b3e65350`, so no in-crate replay re-derives the compact report
/// coordinates or the nested evidence values that feed only the seals. An
/// honestly resealed substitution of those fields replays clean yet diverges
/// the published `(certificate_digest, certificate_report_fingerprint)`
/// identity, so a verifier retaining the authentic pair rejects the
/// substitution. The diverging axis is pinned per case because the two seals
/// do not cover the same inputs: the strong digest chain omits report-only
/// coordinates — `implementation_fragment_count` diverges only the strong
/// digest — while a compact fingerprint inside the entry binding or the text
/// `derivation_report_fingerprint` diverges only the compact fingerprint.
#[test]
fn final_footprint_certificate_compact_fields_stay_identity_bound() {
    let authentic = certificate();
    authentic
        .validate_identity()
        .expect("the authentic certificate replays");

    let stays_identity_bound = |name: &'static str,
                                mutated: &FinalFootprintCertificate,
                                digest_diverges: bool,
                                fingerprint_diverges: bool| {
        assert_ne!(mutated, &authentic, "{name} must change the record");
        mutated.validate_identity().unwrap_or_else(|diagnostic| {
            panic!(
                "{name} stays canonical at the certificate join: {}",
                diagnostic.message
            )
        });
        assert_eq!(
            mutated.certificate_digest != authentic.certificate_digest,
            digest_diverges,
            "{name} must diverge the strong certificate identity as pinned"
        );
        assert_eq!(
            mutated.certificate_report_fingerprint != authentic.certificate_report_fingerprint,
            fingerprint_diverges,
            "{name} must diverge the compact certificate identity as pinned"
        );
    };

    // --- certificate-level coordinates that feed only the seals ---
    let mut mutated = authentic.clone();
    mutated.implementation_fragment_count += 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted implementation fragment count",
        &mutated,
        true,
        false,
    );

    let mut mutated = authentic.clone();
    mutated.callback_placement_identity_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted callback placement coordinate",
        &mutated,
        true,
        true,
    );

    // --- normalized coverage roster substitutions the replay cannot
    // re-derive ---
    let mut mutated = authentic.clone();
    mutated.coverage.covered_classes.remove(0);
    reseal(&mut mutated);
    stays_identity_bound("a dropped covered class", &mutated, true, true);

    let mut mutated = authentic.clone();
    mutated
        .coverage
        .covered_classes
        .insert(1, FinalFootprintClass::CatalogCheckedAssembly);
    reseal(&mut mutated);
    stays_identity_bound("an extended covered roster", &mutated, true, true);

    let mut mutated = authentic.clone();
    mutated.coverage.absent_by_construction_classes.remove(0);
    reseal(&mut mutated);
    stays_identity_bound(
        "a dropped absent-by-construction class",
        &mutated,
        true,
        true,
    );

    let mut mutated = authentic.clone();
    mutated.coverage.final_byte_validated_classes.remove(0);
    reseal(&mut mutated);
    stays_identity_bound("a dropped final-byte-validated class", &mutated, true, true);

    // The validated roster joins no disjointness check: remarking a covered
    // class as also final-byte-validated stays normalized.
    let mut mutated = authentic.clone();
    mutated
        .coverage
        .final_byte_validated_classes
        .push(FinalFootprintClass::AdmittedLeaves);
    reseal(&mut mutated);
    stays_identity_bound(
        "an extended final-byte-validated roster",
        &mutated,
        true,
        true,
    );

    // --- the nested compiler-text validation summary: every imported field
    // feeds `derivation_digest`, so its honestly resealed substitution
    // diverges only the strong certificate identity ---
    let mut mutated = authentic.clone();
    mutated.compiler_text_validation.encoded_text_digest =
        EncodedCompilerTextDigest::from_digest([0xee; 32]);
    reseal(&mut mutated);
    stays_identity_bound("a substituted encoded text digest", &mutated, true, false);

    let mut mutated = authentic.clone();
    mutated.compiler_text_validation.final_compiler_text_digest =
        FinalCompilerTextDigest::from_digest([0xee; 32]);
    reseal(&mut mutated);
    stays_identity_bound("a substituted final text digest", &mutated, true, false);

    let mut mutated = authentic.clone();
    mutated.compiler_text_validation.relocation_envelope_digest =
        CompilerTextRelocationEnvelopeDigest::from_digest([0xee; 32]);
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted relocation envelope digest",
        &mutated,
        true,
        false,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_text_validation
        .encoded_text_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted encoded text fingerprint",
        &mutated,
        true,
        false,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_text_validation
        .final_compiler_text_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted final text fingerprint",
        &mutated,
        true,
        false,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_text_validation
        .relocation_envelope_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted relocation envelope fingerprint",
        &mutated,
        true,
        false,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_text_validation
        .checked_instruction_validation_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted instruction-validation fingerprint",
        &mutated,
        true,
        false,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_text_validation
        .checked_instruction_footprint_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted instruction-footprint fingerprint",
        &mutated,
        true,
        false,
    );

    let mut mutated = authentic.clone();
    mutated.compiler_text_validation.text_relocation_count += 1;
    reseal(&mut mutated);
    stays_identity_bound("a substituted relocation count", &mutated, true, false);

    let mut mutated = authentic.clone();
    mutated
        .compiler_text_validation
        .checked_instruction_validation_count += 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted instruction-validation count",
        &mutated,
        true,
        false,
    );

    // `derivation_report_fingerprint` is the compact twin the strong
    // derivation digest deliberately does not cover.
    let mut mutated = authentic.clone();
    mutated
        .compiler_text_validation
        .derivation_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted text derivation fingerprint",
        &mutated,
        false,
        true,
    );

    // --- the nested compiler-function validation summary feeds
    // `evidence_digest`, and its compact coordinate rehashes that digest, so
    // every substitution diverges both axes ---
    let mut mutated = authentic.clone();
    mutated.compiler_function_validation.function_count += 1;
    reseal(&mut mutated);
    stays_identity_bound("a substituted function count", &mutated, true, true);

    let mut mutated = authentic.clone();
    mutated.compiler_function_validation.instruction_count += 1;
    reseal(&mut mutated);
    stays_identity_bound("a substituted instruction count", &mutated, true, true);

    let mut mutated = authentic.clone();
    mutated
        .compiler_function_validation
        .zero_width_instruction_count += 1;
    reseal(&mut mutated);
    stays_identity_bound("a substituted zero-width count", &mutated, true, true);

    let mut mutated = authentic.clone();
    mutated
        .compiler_function_validation
        .frame_prologue_byte_count += 1;
    reseal(&mut mutated);
    stays_identity_bound("a substituted prologue byte count", &mutated, true, true);

    let mut mutated = authentic.clone();
    mutated
        .compiler_function_validation
        .frame_epilogue_byte_count += 1;
    reseal(&mut mutated);
    stays_identity_bound("a substituted epilogue byte count", &mutated, true, true);

    let mut mutated = authentic.clone();
    mutated
        .compiler_function_validation
        .fragment_manifest_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted fragment manifest coordinate",
        &mutated,
        true,
        true,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_function_validation
        .frame_application_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted frame application coordinate",
        &mutated,
        true,
        true,
    );

    // The boundary join is one-directional: the function report may omit
    // the boundary coordinate while the certificate retains exact custody.
    let mut mutated = authentic.clone();
    mutated
        .compiler_function_validation
        .boundary_contract_report_fingerprint = None;
    reseal(&mut mutated);
    stays_identity_bound(
        "a function report omitting the boundary coordinate",
        &mutated,
        true,
        true,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_function_validation
        .final_text_validation_report_fingerprint ^= 1;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted final text validation coordinate",
        &mutated,
        true,
        true,
    );

    // --- entry-binding fields the joins do not re-derive: digests diverge
    // only the strong identity, report fingerprints only the compact one ---
    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .entry_region_evidence_digest =
        crate::CompilerEntryRegionBindingDigest::from_digest([0xee; 32]);
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted entry-region evidence digest",
        &mutated,
        true,
        false,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .entry_region_evidence_report_fingerprint = 21;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted entry-region evidence fingerprint",
        &mutated,
        false,
        true,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .prior_inventory_digest =
        crate::PlacedExecutableRegionInventoryDigest::from_digest([13; 32]);
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted prior inventory digest",
        &mutated,
        true,
        false,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .prior_inventory_report_fingerprint = 13;
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted prior inventory fingerprint",
        &mutated,
        false,
        true,
    );

    let mut mutated = authentic.clone();
    mutated
        .compiler_entry_footprint_binding
        .as_mut()
        .expect("entry binding")
        .footprint_digest = crate::StateFootprintEvidenceDigest::from_digest([0xee; 32]);
    reseal(&mut mutated);
    stays_identity_bound(
        "a substituted entry footprint digest",
        &mutated,
        true,
        false,
    );
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
