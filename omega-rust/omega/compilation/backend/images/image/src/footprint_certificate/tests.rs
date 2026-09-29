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
use diagnostics::Diagnostic;
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
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

/// An authentic foreign certificate — a gap-free foreign inventory under a
/// certificate retaining no entry-footprint custody — that supplies donor
/// custody for the "foreign" legs and proves the donor carries real evidence
/// of its own.
fn foreign_certificate() -> FinalFootprintCertificate {
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
            symbol: "foreign_entry".into(),
            footprint: None,
        });
    let inventory =
        crate::place_executable_regions(&foreign_image, crate::FinalImageLayout::default())
            .expect("foreign inventory places");
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
    .expect("foreign certificate")
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

/// Every replay-rejecting field of [`FinalFootprintCertificate`] substitutes
/// independently and rejects at `validate_identity` replay under an honestly
/// recomputed containing identity — the declared
/// `FinalFootprintCertificateFieldForTest` inventory run through
/// `run_one_field_substitution_matrix`, which pins each leg's exact refusal:
/// the marker, the boundary-contract and implementation-evidence joins into
/// the nested function validation and entry-footprint binding, every join
/// and well-formedness field of the binding itself, and every inventory axis
/// through the inventory digest seal plus the resulting-inventory custody
/// join. Coverage flag and roster substitutions that cannot keep the
/// normalized disjoint shape reject at the record's normalization admission.
/// Fields the replay cannot re-derive — compact report coordinates inside
/// the nested evidence and `implementation_fragment_count` — stay canonical
/// at this join and remain bound by the published certificate identity; see
/// `final_footprint_certificate_compact_fields_stay_identity_bound`. The
/// retained seals stay authored explicitly below — a stale seal is itself
/// the substitution — as do the two no-entry custody legs that need the
/// binding-free fixture rather than the honest record.
#[test]
fn final_footprint_certificate_rejects_every_one_field_substitution() {
    let authentic = certificate();
    authentic
        .validate_identity()
        .expect("the authentic certificate replays");
    assert_eq!(authentic.inventory.regions.len(), 2);

    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "final footprint certificate",
        fields: FinalFootprintCertificateFieldForTest::INVENTORY,
        honest: &certificate,
        donor: foreign_certificate(),
        custody: &Clone::clone,
        substitute: &|certificate, field, donor| {
            substitute_final_footprint_certificate(certificate, field, donor);
        },
        check: &|certificate| {
            certificate
                .validate_identity()
                .map(|()| certificate.clone())
        },
        outcome: &final_footprint_certificate_outcome,
        joined_replay: None,
    });

    let rejects_at_replay = |name: &'static str, mutated: &FinalFootprintCertificate| {
        assert_ne!(mutated, &authentic, "{name} must change the record");
        assert!(
            mutated.validate_identity().is_err(),
            "{name} must reject at independent replay"
        );
    };

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
/// identity — the declared
/// `FinalFootprintCertificateCompactFieldForTest` inventory run through
/// `run_one_field_substitution_matrix`, where the checker replays the
/// substituted record and answers with the authentic retained identity so
/// `RebuiltCustodyDiffers` requires divergence. `joined_replay` pins the
/// diverging axis per case because the two seals do not cover the same
/// inputs: the strong digest chain omits report-only coordinates —
/// `implementation_fragment_count` diverges only the strong digest — while a
/// compact fingerprint inside the entry binding or the text
/// `derivation_report_fingerprint` diverges only the compact fingerprint.
#[test]
fn final_footprint_certificate_compact_fields_stay_identity_bound() {
    let authentic = certificate();
    authentic
        .validate_identity()
        .expect("the authentic certificate replays");
    let authentic_identity = (
        authentic.certificate_digest,
        authentic.certificate_report_fingerprint,
    );

    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "final footprint certificate compact fields",
        fields: FinalFootprintCertificateCompactFieldForTest::INVENTORY,
        honest: &certificate,
        donor: foreign_certificate(),
        custody: &|certificate: &FinalFootprintCertificate| {
            (
                certificate.certificate_digest,
                certificate.certificate_report_fingerprint,
            )
        },
        substitute: &|certificate, field, _donor| {
            substitute_final_footprint_certificate_compact(certificate, field);
        },
        check: &|certificate| certificate.validate_identity().map(|()| authentic_identity),
        outcome: &|_| MutationOutcome::RebuiltCustodyDiffers,
        joined_replay: Some(&|certificate: &FinalFootprintCertificate, field| {
            let (digest_diverges, fingerprint_diverges) = compact_identity_axes(field);
            assert_eq!(
                certificate.certificate_digest != authentic_identity.0,
                digest_diverges,
                "{field:?} must diverge the strong certificate identity as pinned"
            );
            assert_eq!(
                certificate.certificate_report_fingerprint != authentic_identity.1,
                fingerprint_diverges,
                "{field:?} must diverge the compact certificate identity as pinned"
            );
        }),
    });
}

optimization_core::custody_field_inventory! {
    /// One substitutable field or roster axis of [`FinalFootprintCertificate`]
    /// whose honest reseal still rejects at `validate_identity`: the marker,
    /// certificate-level join coordinates, every coverage flag and roster
    /// mutation that breaks the normalized disjoint shape, the nested
    /// function-report joins, the entry-footprint binding's presence and
    /// join coordinates, and every inventory axis the resulting-inventory
    /// custody join binds. The retained seals are absent: a stale seal is
    /// itself the substitution and stays authored explicitly in the test, as
    /// do the no-entry custody legs built on the binding-free fixture.
    pub enum FinalFootprintCertificateFieldForTest {
        SubstitutedMarker,
        SubstitutedBoundaryContractCoordinate,
        DroppedBoundaryContractCoordinate,
        SubstitutedImplementationEvidenceCoordinate,
        DroppedEnumerationCompletenessClaim,
        DroppedRegionEnumerationClaim,
        DroppedFootprintEnumerationClaim,
        ReorderedCoveredRoster,
        DuplicatedCoveredClass,
        CoveredClassAbsentByConstruction,
        ReorderedAbsentByConstructionRoster,
        AbsentByConstructionClassCovered,
        ReorderedFinalByteValidatedRoster,
        DuplicatedFinalByteValidatedClass,
        MissingClassUnderCompleteEnumeration,
        MissingClassAlsoCovered,
        FunctionReportNamingDifferentBoundaryContract,
        FunctionReportNamingDifferentRegionBinding,
        DroppedEntryFootprintBinding,
        BindingNamingDifferentRegionBinding,
        BindingWithZeroRegionBindingCoordinate,
        BindingNamingDifferentImplementationEvidence,
        BindingWithZeroFootprintCoordinate,
        BindingNamingDifferentResultingInventory,
        BindingNamingDifferentResultingInventoryFingerprint,
        BindingPriorInventoryEqualsResult,
        BindingPriorInventoryFingerprintEqualsResult,
        BindingWithZeroPriorInventoryCoordinate,
        BindingWithZeroEntryRegionCoordinate,
        SubstitutedInventoryTextBase,
        ExtendedInventoryTextExtent,
        ForeignInventoryTextDigest,
        SubstitutedInventoryTextFingerprint,
        ShiftedRegionSectionOffset,
        SubstitutedRegionAddress,
        ExtendedRegionByteCount,
        ForeignRegionByteDigest,
        SubstitutedRegionByteFingerprint,
        SubstitutedRegionSymbol,
        RemarkedRegionOrigin,
        DroppedRegionFootprint,
        AddedRegionFootprint,
        DroppedRegionRow,
        ReorderedRegionRoster,
        DuplicatedRegionRow,
        ForeignRegionRow,
        RetainedExecutableGap,
        ForeignInventory,
    }
}

/// Substitute exactly one declared field — drawing authentic foreign custody
/// from `donor` where a leg needs it — and honestly reseal every containing
/// seal, so only the independent `validate_identity` replay can expose the
/// leg.
fn substitute_final_footprint_certificate(
    certificate: &mut FinalFootprintCertificate,
    field: FinalFootprintCertificateFieldForTest,
    donor: &FinalFootprintCertificate,
) {
    use FinalFootprintCertificateFieldForTest as Field;
    match field {
        // The marker is compared verbatim; the seals hash the constant.
        Field::SubstitutedMarker => {
            certificate.marker = "omega.final-footprint-certificate.stale";
        }
        // --- certificate scalar coordinates joined into nested evidence ---
        Field::SubstitutedBoundaryContractCoordinate => {
            certificate.boundary_contract_report_fingerprint = Some(99);
        }
        Field::DroppedBoundaryContractCoordinate => {
            certificate.boundary_contract_report_fingerprint = None;
        }
        Field::SubstitutedImplementationEvidenceCoordinate => {
            certificate.implementation_evidence_report_fingerprint = 99;
        }
        // --- coverage flags: a single-flag substitution cannot keep the
        // completeness invariant, so each rejects at normalization admission ---
        Field::DroppedEnumerationCompletenessClaim => {
            certificate.coverage.enumeration_complete = false;
        }
        Field::DroppedRegionEnumerationClaim => {
            certificate.coverage.region_enumeration_complete = false;
        }
        Field::DroppedFootprintEnumerationClaim => {
            certificate.coverage.footprint_enumeration_complete = false;
        }
        // --- coverage rosters: order, duplication, and disjointness are the
        // normalized shape; violations reject at normalization admission ---
        Field::ReorderedCoveredRoster => certificate.coverage.covered_classes.swap(0, 1),
        Field::DuplicatedCoveredClass => {
            certificate
                .coverage
                .covered_classes
                .insert(1, FinalFootprintClass::CompilerFunctions);
        }
        Field::CoveredClassAbsentByConstruction => {
            certificate
                .coverage
                .covered_classes
                .push(FinalFootprintClass::AdmittedLeaves);
        }
        Field::ReorderedAbsentByConstructionRoster => {
            certificate
                .coverage
                .absent_by_construction_classes
                .swap(0, 1);
        }
        Field::AbsentByConstructionClassCovered => {
            certificate
                .coverage
                .absent_by_construction_classes
                .insert(0, FinalFootprintClass::CompilerFunctions);
        }
        Field::ReorderedFinalByteValidatedRoster => {
            certificate.coverage.final_byte_validated_classes.swap(0, 1);
        }
        Field::DuplicatedFinalByteValidatedClass => {
            certificate
                .coverage
                .final_byte_validated_classes
                .insert(0, FinalFootprintClass::CatalogCheckedAssembly);
        }
        // A missing class that conflicts with no other roster still rejects:
        // a complete footprint enumeration cannot retain missing classes.
        Field::MissingClassUnderCompleteEnumeration => {
            certificate
                .coverage
                .missing_classes
                .push(FinalFootprintClass::CatalogCheckedAssembly);
        }
        Field::MissingClassAlsoCovered => {
            certificate
                .coverage
                .missing_classes
                .push(FinalFootprintClass::CompilerFunctions);
        }
        // --- nested compiler-function validation joins ---
        Field::FunctionReportNamingDifferentBoundaryContract => {
            certificate
                .compiler_function_validation
                .boundary_contract_report_fingerprint = Some(99);
        }
        Field::FunctionReportNamingDifferentRegionBinding => {
            certificate
                .compiler_function_validation
                .final_region_binding_report_fingerprint = 99;
        }
        // --- entry-footprint binding presence and every join coordinate ---
        Field::DroppedEntryFootprintBinding => {
            certificate.compiler_entry_footprint_binding = None;
        }
        Field::BindingNamingDifferentRegionBinding => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .final_region_binding_report_fingerprint = 99;
        }
        Field::BindingWithZeroRegionBindingCoordinate => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .final_region_binding_report_fingerprint = 0;
        }
        Field::BindingNamingDifferentImplementationEvidence => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .footprint_report_fingerprint = 99;
        }
        Field::BindingWithZeroFootprintCoordinate => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .footprint_report_fingerprint = 0;
        }
        Field::BindingNamingDifferentResultingInventory => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .resulting_inventory_digest = donor.inventory.inventory_digest;
        }
        Field::BindingNamingDifferentResultingInventoryFingerprint => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .resulting_inventory_report_fingerprint ^= 1;
        }
        // The binding is the sole authorized mutation record: a prior
        // identity equal to the resulting one claims the footprint
        // attachment changed nothing.
        Field::BindingPriorInventoryEqualsResult => {
            let resulting = certificate
                .compiler_entry_footprint_binding
                .expect("entry binding")
                .resulting_inventory_digest;
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .prior_inventory_digest = resulting;
        }
        Field::BindingPriorInventoryFingerprintEqualsResult => {
            let resulting = certificate
                .compiler_entry_footprint_binding
                .expect("entry binding")
                .resulting_inventory_report_fingerprint;
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .prior_inventory_report_fingerprint = resulting;
        }
        Field::BindingWithZeroPriorInventoryCoordinate => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .prior_inventory_report_fingerprint = 0;
        }
        Field::BindingWithZeroEntryRegionCoordinate => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .entry_region_evidence_report_fingerprint = 0;
        }
        // --- the retained inventory: every content substitution diverges
        // the inventory identity the entry-footprint custody join binds; the
        // byte-level partition replay remains the inventory's own lane ---
        Field::SubstitutedInventoryTextBase => certificate.inventory.text_address += 0x1000,
        Field::ExtendedInventoryTextExtent => certificate.inventory.text_byte_count += 4,
        Field::ForeignInventoryTextDigest => {
            certificate.inventory.text_digest = donor.inventory.text_digest;
        }
        Field::SubstitutedInventoryTextFingerprint => {
            certificate.inventory.text_report_fingerprint ^= 1;
        }
        Field::ShiftedRegionSectionOffset => certificate.inventory.regions[0].section_offset = 1,
        Field::SubstitutedRegionAddress => certificate.inventory.regions[0].address += 1,
        Field::ExtendedRegionByteCount => certificate.inventory.regions[0].byte_count = 8,
        Field::ForeignRegionByteDigest => {
            certificate.inventory.regions[0].byte_digest = donor.inventory.regions[0].byte_digest;
        }
        Field::SubstitutedRegionByteFingerprint => {
            certificate.inventory.regions[0].byte_report_fingerprint ^= 1;
        }
        Field::SubstitutedRegionSymbol => {
            certificate.inventory.regions[0].symbol = "forged".into();
        }
        Field::RemarkedRegionOrigin => {
            certificate.inventory.regions[0].origin =
                crate::FinalExecutableRegionOrigin::ImportThunk;
        }
        Field::DroppedRegionFootprint => certificate.inventory.regions[0].footprint = None,
        Field::AddedRegionFootprint => {
            certificate.inventory.regions[1].footprint = Some(StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rcx]),
                MachineStateSet::new([MachineState::SegmentState]),
            ));
        }
        Field::DroppedRegionRow => {
            certificate.inventory.regions.remove(0);
        }
        Field::ReorderedRegionRoster => certificate.inventory.regions.swap(0, 1),
        Field::DuplicatedRegionRow => {
            let row = certificate.inventory.regions[0].clone();
            certificate.inventory.regions.push(row);
        }
        Field::ForeignRegionRow => {
            let row = donor.inventory.regions[0].clone();
            certificate.inventory.regions.insert(1, row);
        }
        // A region-complete certificate cannot retain an executable gap;
        // gap row fields are not representable inside one and are covered by
        // the inventory's own byte-level substitution matrix.
        Field::RetainedExecutableGap => {
            certificate
                .inventory
                .unclassified_gaps
                .push(crate::PlacedExecutableGap {
                    section_offset: 8,
                    address: 0x1008,
                    byte_count: 1,
                    byte_digest: crate::PlacedExecutableGapBytesDigest::from_digest([1; 32]),
                    byte_report_fingerprint: 1,
                });
        }
        // A whole-inventory substitution: the foreign inventory is honestly
        // sealed by placement yet is not the inventory the binding binds.
        Field::ForeignInventory => certificate.inventory = donor.inventory.clone(),
    }
    reseal(certificate);
}

/// The exact replay refusal each declared field pins at `validate_identity`:
/// marker, normalization admission, nested evidence joins, binding custody,
/// and the certificate's own checks, in the replay's check order.
fn final_footprint_certificate_outcome(
    field: FinalFootprintCertificateFieldForTest,
) -> MutationOutcome<Diagnostic> {
    use FinalFootprintCertificateFieldForTest as Field;
    MutationOutcome::ExactError(Diagnostic::error(match field {
        Field::SubstitutedMarker => "unsupported final footprint certificate marker".to_string(),
        Field::SubstitutedBoundaryContractCoordinate
        | Field::DroppedBoundaryContractCoordinate
        | Field::FunctionReportNamingDifferentBoundaryContract => {
            "final compiler-function report names a different boundary contract".to_string()
        }
        Field::SubstitutedImplementationEvidenceCoordinate
        | Field::FunctionReportNamingDifferentRegionBinding
        | Field::BindingNamingDifferentRegionBinding
        | Field::BindingWithZeroRegionBindingCoordinate
        | Field::BindingNamingDifferentImplementationEvidence
        | Field::BindingWithZeroFootprintCoordinate
        | Field::BindingNamingDifferentResultingInventory
        | Field::BindingNamingDifferentResultingInventoryFingerprint
        | Field::BindingPriorInventoryEqualsResult
        | Field::BindingPriorInventoryFingerprintEqualsResult
        | Field::BindingWithZeroPriorInventoryCoordinate
        | Field::BindingWithZeroEntryRegionCoordinate
        | Field::SubstitutedInventoryTextBase
        | Field::ExtendedInventoryTextExtent
        | Field::ForeignInventoryTextDigest
        | Field::SubstitutedInventoryTextFingerprint
        | Field::ShiftedRegionSectionOffset
        | Field::SubstitutedRegionAddress
        | Field::ExtendedRegionByteCount
        | Field::ForeignRegionByteDigest
        | Field::SubstitutedRegionByteFingerprint
        | Field::SubstitutedRegionSymbol
        | Field::RemarkedRegionOrigin
        | Field::DroppedRegionFootprint
        | Field::AddedRegionFootprint
        | Field::DroppedRegionRow
        | Field::ReorderedRegionRoster
        | Field::DuplicatedRegionRow
        | Field::ForeignRegionRow
        | Field::ForeignInventory => {
            "final footprint certificate entry-footprint mutation custody drifted".to_string()
        }
        Field::DroppedEntryFootprintBinding => {
            "final footprint certificate lacks exact entry-footprint mutation custody".to_string()
        }
        Field::DroppedEnumerationCompletenessClaim
        | Field::DroppedRegionEnumerationClaim
        | Field::DroppedFootprintEnumerationClaim => {
            "final footprint certificate completeness flags disagree".to_string()
        }
        Field::ReorderedCoveredRoster | Field::DuplicatedCoveredClass => {
            "final footprint certificate covered classes are not strictly normalized".to_string()
        }
        Field::CoveredClassAbsentByConstruction => {
            "final footprint class `admitted_leaves` is both covered and absent-by-construction"
                .to_string()
        }
        Field::ReorderedAbsentByConstructionRoster => {
            "final footprint certificate absent-by-construction classes are not strictly normalized"
                .to_string()
        }
        Field::AbsentByConstructionClassCovered => {
            "final footprint class `compiler_functions` is both covered and absent-by-construction"
                .to_string()
        }
        Field::ReorderedFinalByteValidatedRoster | Field::DuplicatedFinalByteValidatedClass => {
            "final footprint certificate final-byte-validated classes are not strictly normalized"
                .to_string()
        }
        Field::MissingClassUnderCompleteEnumeration => {
            "complete final footprint enumeration cannot retain missing classes".to_string()
        }
        Field::MissingClassAlsoCovered => {
            "final footprint class `compiler_functions` is both covered and missing".to_string()
        }
        Field::RetainedExecutableGap => {
            "region-complete final footprint certificate retains executable gaps".to_string()
        }
    }))
}

optimization_core::custody_field_inventory! {
    /// A field substitution the certificate replay cannot re-derive: the
    /// record stays canonical at `validate_identity` under an honest reseal,
    /// and the substitution is exposed only by the diverging published
    /// `(certificate_digest, certificate_report_fingerprint)` identity.
    /// `joined_replay` pins which of the two published axes each leg
    /// diverges, because the strong digest chain and the compact fingerprint
    /// chain do not cover the same inputs.
    pub enum FinalFootprintCertificateCompactFieldForTest {
        SubstitutedImplementationFragmentCount,
        SubstitutedCallbackPlacementCoordinate,
        DroppedCoveredClass,
        ExtendedCoveredRoster,
        DroppedAbsentByConstructionClass,
        DroppedFinalByteValidatedClass,
        ExtendedFinalByteValidatedRoster,
        SubstitutedEncodedTextDigest,
        SubstitutedFinalTextDigest,
        SubstitutedRelocationEnvelopeDigest,
        SubstitutedEncodedTextFingerprint,
        SubstitutedFinalTextFingerprint,
        SubstitutedRelocationEnvelopeFingerprint,
        SubstitutedInstructionValidationFingerprint,
        SubstitutedInstructionFootprintFingerprint,
        SubstitutedRelocationCount,
        SubstitutedInstructionValidationCount,
        SubstitutedTextDerivationFingerprint,
        SubstitutedFunctionCount,
        SubstitutedInstructionCount,
        SubstitutedZeroWidthCount,
        SubstitutedPrologueByteCount,
        SubstitutedEpilogueByteCount,
        SubstitutedFragmentManifestCoordinate,
        SubstitutedFrameApplicationCoordinate,
        FunctionReportOmittingBoundaryCoordinate,
        SubstitutedFinalTextValidationCoordinate,
        SubstitutedEntryRegionEvidenceDigest,
        SubstitutedEntryRegionEvidenceFingerprint,
        SubstitutedPriorInventoryDigest,
        SubstitutedPriorInventoryFingerprint,
        SubstitutedEntryFootprintDigest,
    }
}

/// Substitute one compact-coordinate field and honestly reseal every
/// containing seal: the certificate replay stays canonical, so only the
/// diverging published identity exposes the substitution.
fn substitute_final_footprint_certificate_compact(
    certificate: &mut FinalFootprintCertificate,
    field: FinalFootprintCertificateCompactFieldForTest,
) {
    use FinalFootprintCertificateCompactFieldForTest as Field;
    match field {
        // --- certificate-level coordinates that feed only the seals ---
        Field::SubstitutedImplementationFragmentCount => {
            certificate.implementation_fragment_count += 1;
        }
        Field::SubstitutedCallbackPlacementCoordinate => {
            certificate.callback_placement_identity_report_fingerprint ^= 1;
        }
        // --- normalized coverage roster substitutions the replay cannot
        // re-derive ---
        Field::DroppedCoveredClass => {
            certificate.coverage.covered_classes.remove(0);
        }
        Field::ExtendedCoveredRoster => {
            certificate
                .coverage
                .covered_classes
                .insert(1, FinalFootprintClass::CatalogCheckedAssembly);
        }
        Field::DroppedAbsentByConstructionClass => {
            certificate
                .coverage
                .absent_by_construction_classes
                .remove(0);
        }
        Field::DroppedFinalByteValidatedClass => {
            certificate.coverage.final_byte_validated_classes.remove(0);
        }
        // The validated roster joins no disjointness check: remarking a
        // covered class as also final-byte-validated stays normalized.
        Field::ExtendedFinalByteValidatedRoster => {
            certificate
                .coverage
                .final_byte_validated_classes
                .push(FinalFootprintClass::AdmittedLeaves);
        }
        // --- the nested compiler-text validation summary: every imported
        // field feeds `derivation_digest`, so its honestly resealed
        // substitution diverges only the strong certificate identity ---
        Field::SubstitutedEncodedTextDigest => {
            certificate.compiler_text_validation.encoded_text_digest =
                EncodedCompilerTextDigest::from_digest([0xee; 32]);
        }
        Field::SubstitutedFinalTextDigest => {
            certificate
                .compiler_text_validation
                .final_compiler_text_digest = FinalCompilerTextDigest::from_digest([0xee; 32]);
        }
        Field::SubstitutedRelocationEnvelopeDigest => {
            certificate
                .compiler_text_validation
                .relocation_envelope_digest =
                CompilerTextRelocationEnvelopeDigest::from_digest([0xee; 32]);
        }
        Field::SubstitutedEncodedTextFingerprint => {
            certificate
                .compiler_text_validation
                .encoded_text_report_fingerprint ^= 1;
        }
        Field::SubstitutedFinalTextFingerprint => {
            certificate
                .compiler_text_validation
                .final_compiler_text_report_fingerprint ^= 1;
        }
        Field::SubstitutedRelocationEnvelopeFingerprint => {
            certificate
                .compiler_text_validation
                .relocation_envelope_report_fingerprint ^= 1;
        }
        Field::SubstitutedInstructionValidationFingerprint => {
            certificate
                .compiler_text_validation
                .checked_instruction_validation_report_fingerprint ^= 1;
        }
        Field::SubstitutedInstructionFootprintFingerprint => {
            certificate
                .compiler_text_validation
                .checked_instruction_footprint_report_fingerprint ^= 1;
        }
        Field::SubstitutedRelocationCount => {
            certificate.compiler_text_validation.text_relocation_count += 1;
        }
        Field::SubstitutedInstructionValidationCount => {
            certificate
                .compiler_text_validation
                .checked_instruction_validation_count += 1;
        }
        // `derivation_report_fingerprint` is the compact twin the strong
        // derivation digest deliberately does not cover.
        Field::SubstitutedTextDerivationFingerprint => {
            certificate
                .compiler_text_validation
                .derivation_report_fingerprint ^= 1;
        }
        // --- the nested compiler-function validation summary feeds
        // `evidence_digest`, and its compact coordinate rehashes that
        // digest, so every substitution diverges both axes ---
        Field::SubstitutedFunctionCount => {
            certificate.compiler_function_validation.function_count += 1;
        }
        Field::SubstitutedInstructionCount => {
            certificate.compiler_function_validation.instruction_count += 1;
        }
        Field::SubstitutedZeroWidthCount => {
            certificate
                .compiler_function_validation
                .zero_width_instruction_count += 1;
        }
        Field::SubstitutedPrologueByteCount => {
            certificate
                .compiler_function_validation
                .frame_prologue_byte_count += 1;
        }
        Field::SubstitutedEpilogueByteCount => {
            certificate
                .compiler_function_validation
                .frame_epilogue_byte_count += 1;
        }
        Field::SubstitutedFragmentManifestCoordinate => {
            certificate
                .compiler_function_validation
                .fragment_manifest_report_fingerprint ^= 1;
        }
        Field::SubstitutedFrameApplicationCoordinate => {
            certificate
                .compiler_function_validation
                .frame_application_report_fingerprint ^= 1;
        }
        // The boundary join is one-directional: the function report may
        // omit the boundary coordinate while the certificate retains exact
        // custody.
        Field::FunctionReportOmittingBoundaryCoordinate => {
            certificate
                .compiler_function_validation
                .boundary_contract_report_fingerprint = None;
        }
        Field::SubstitutedFinalTextValidationCoordinate => {
            certificate
                .compiler_function_validation
                .final_text_validation_report_fingerprint ^= 1;
        }
        // --- entry-binding fields the joins do not re-derive: digests
        // diverge only the strong identity, report fingerprints only the
        // compact one ---
        Field::SubstitutedEntryRegionEvidenceDigest => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .entry_region_evidence_digest =
                crate::CompilerEntryRegionBindingDigest::from_digest([0xee; 32]);
        }
        Field::SubstitutedEntryRegionEvidenceFingerprint => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .entry_region_evidence_report_fingerprint = 21;
        }
        Field::SubstitutedPriorInventoryDigest => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .prior_inventory_digest =
                crate::PlacedExecutableRegionInventoryDigest::from_digest([13; 32]);
        }
        Field::SubstitutedPriorInventoryFingerprint => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .prior_inventory_report_fingerprint = 13;
        }
        Field::SubstitutedEntryFootprintDigest => {
            certificate
                .compiler_entry_footprint_binding
                .as_mut()
                .expect("entry binding")
                .footprint_digest = crate::StateFootprintEvidenceDigest::from_digest([0xee; 32]);
        }
    }
    reseal(certificate);
}

/// The `(strong digest diverges, compact fingerprint diverges)` pin each
/// compact-field leg carries: the strong `derivation_digest` /
/// `evidence_digest` chains omit report-only coordinates, while the compact
/// `derivation_report_fingerprint` / `evidence_report_fingerprint` chains
/// omit the strong digests.
fn compact_identity_axes(field: FinalFootprintCertificateCompactFieldForTest) -> (bool, bool) {
    use FinalFootprintCertificateCompactFieldForTest as Field;
    match field {
        Field::SubstitutedImplementationFragmentCount
        | Field::SubstitutedEncodedTextDigest
        | Field::SubstitutedFinalTextDigest
        | Field::SubstitutedRelocationEnvelopeDigest
        | Field::SubstitutedEncodedTextFingerprint
        | Field::SubstitutedFinalTextFingerprint
        | Field::SubstitutedRelocationEnvelopeFingerprint
        | Field::SubstitutedInstructionValidationFingerprint
        | Field::SubstitutedInstructionFootprintFingerprint
        | Field::SubstitutedRelocationCount
        | Field::SubstitutedInstructionValidationCount
        | Field::SubstitutedEntryRegionEvidenceDigest
        | Field::SubstitutedPriorInventoryDigest
        | Field::SubstitutedEntryFootprintDigest => (true, false),
        Field::SubstitutedTextDerivationFingerprint
        | Field::SubstitutedEntryRegionEvidenceFingerprint
        | Field::SubstitutedPriorInventoryFingerprint => (false, true),
        Field::SubstitutedCallbackPlacementCoordinate
        | Field::DroppedCoveredClass
        | Field::ExtendedCoveredRoster
        | Field::DroppedAbsentByConstructionClass
        | Field::DroppedFinalByteValidatedClass
        | Field::ExtendedFinalByteValidatedRoster
        | Field::SubstitutedFunctionCount
        | Field::SubstitutedInstructionCount
        | Field::SubstitutedZeroWidthCount
        | Field::SubstitutedPrologueByteCount
        | Field::SubstitutedEpilogueByteCount
        | Field::SubstitutedFragmentManifestCoordinate
        | Field::SubstitutedFrameApplicationCoordinate
        | Field::FunctionReportOmittingBoundaryCoordinate
        | Field::SubstitutedFinalTextValidationCoordinate => (true, true),
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
