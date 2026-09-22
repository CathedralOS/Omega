use super::installation_field_substitution_fields::{
    foreign_stored_dynamic_call_record, installation_record_check, installation_record_custody,
};
use super::{
    TestComponentProgressAcceptance, WriteExitProvider, account_x86_unit_call, artifact_symbol,
    edge_id, identity, integer_return, internal_call_plan, machine_id, operation_id,
    port_effect_plan, structural_return_plan, two_call_edge_owned_cleanup_plan, two_function_plan,
};
use calling_conventions::{ValuePlacement, ValueShape};
use image_emission::{
    INSTALLATION_FORMAT_MARKER, InstallationError, InstallationRecord, build_installation_record,
    build_installation_record_with_evidence,
    build_installation_record_with_selected_provider_plans_and_evidence, build_object_artifact,
    decode_installation_record, derive_installation_stack_demand, emit_direct_executable_image,
    encode_installation_record, installation_fingerprint, validate_installation_record,
};
use installation_evidence::ProviderExecutionEvidence;
use machine_code::{MachineCodeFunction, MachineCodePlan};
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
};
use semantic_vocabulary::{ClaimId, MachineId, PlaceId, ProfileDecisionId, StructuralTypeId};
use target::NativeTarget;
use target_operations::{CallSiteOwner, TerminalPsiProvenance};
use terminal_psi::{
    SemanticFingerprint, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    VocabularyMarker,
};

#[test]
fn supported_writers_preserve_exact_terminal_text_and_complete_regions() {
    let targets = [
        (NativeTarget::linux_x64(), b"\x7fELF".as_slice()),
        (NativeTarget::linux_arm64(), b"\x7fELF".as_slice()),
        (NativeTarget::macos_arm64(), b"\xcf\xfa\xed\xfe".as_slice()),
        (NativeTarget::windows_x64(), b"MZ".as_slice()),
    ];

    for (target, magic) in targets {
        let bytes = match target.architecture {
            target::Architecture::X86_64 => integer_return(7),
            target::Architecture::Aarch64 => {
                vec![0xe0, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
            }
        };
        let machine = machine_id(1);
        let plan = MachineCodePlan {
            psi: identity(),
            target,
            entry: machine,
            functions: vec![MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: None,
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: bytes.clone(),
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: None,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: Vec::new(),
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                unit_continuations: Vec::new(),
                unit_affine_cleanup: None,
                semantic_code_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            }],
        };
        let artifact = build_object_artifact(&plan).expect("artifact");
        let image = emit_direct_executable_image(&artifact, 3)
            .unwrap_or_else(|error| panic!("{target:?} image failed: {error}"));
        assert_eq!(image.psi(), plan.psi);
        let installation = build_installation_record(&image, ProfileDecisionId::new(1).unwrap())
            .expect("installation record");
        assert_eq!(
            installation.subsystem(),
            matches!(target.object_format, target::ObjectFormat::Coff).then_some(3)
        );
        let installation_bytes =
            encode_installation_record(&installation).expect("installation bytes");
        assert_eq!(
            decode_installation_record(&installation_bytes),
            Ok(installation)
        );
        let image = image.output();

        assert!(image.bytes.starts_with(magic), "{target:?} image magic");
        assert_eq!(image.final_text_bytes, bytes, "{target:?} final text");
        assert_eq!(image.text_bytes, bytes.len());
        assert_eq!(image.relocations, 0);
        assert_eq!(image.final_image_imports, 0);
        assert_eq!(image.final_image_relocations, 0);
        assert!(image.executable_regions.unclassified_gaps.is_empty());
        assert_eq!(image.executable_regions.regions.len(), 1);
        assert_eq!(
            image.executable_regions.regions[0].symbol,
            artifact_symbol(&artifact)
        );
        let evidence = image
            .compiler_text_validation
            .expect("exact terminal text should publish validation evidence");
        assert_eq!(
            evidence.encoded_text_report_fingerprint,
            evidence.final_compiler_text_report_fingerprint
        );
        assert!(evidence.has_valid_derivation_digest());
        assert_ne!(
            evidence.encoded_text_digest.as_bytes(),
            evidence.final_compiler_text_digest.as_bytes(),
            "distinct digest domains remain separate even for identical bytes"
        );
        assert_eq!(evidence.text_relocation_count, 0);
        assert_eq!(evidence.checked_instruction_validation_count, 0);
    }
}

#[test]
fn installation_record_is_canonical_and_binds_exact_image_and_target_facts() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("Linux image");
    let record = build_installation_record(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
    )
    .expect("installation record");

    assert_eq!(record.psi(), plan.psi);
    assert_eq!(record.target(), plan.target);
    assert_eq!(record.subsystem(), None);
    assert!(record.selected_provider_plans().is_empty());
    let bytes = encode_installation_record(&record).expect("canonical bytes");
    assert_eq!(&bytes[..8], b"PSIINST\0");
    assert_eq!(
        u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        INSTALLATION_FORMAT_MARKER
    );
    assert_eq!(decode_installation_record(&bytes), Ok(record.clone()));
    validate_installation_record(&record, &image).expect("exact image binding");
    // Format 99 retains the by-value opaque boundary application's selected
    // custody on top of format 98's referent path segments, reference
    // structural type shapes, and result source rosters. The marker is checked
    // before the body, so a relabeled payload cannot masquerade as a
    // predecessor format.
    // Reconstruct framing independently of the production helper and pin both
    // identities.
    use sha2::{Digest, Sha256};
    let independent_fingerprint = |payload: &[u8]| {
        let mut digest = Sha256::new();
        digest.update(b"omega-installation-record\0");
        digest.update(u64::try_from(payload.len()).unwrap().to_le_bytes());
        digest.update(payload);
        format!("{:x}", digest.finalize())
    };
    let mut predecessor_payload = bytes.clone();
    predecessor_payload[8..10].copy_from_slice(&95_u16.to_le_bytes());
    assert_eq!(
        independent_fingerprint(&predecessor_payload),
        "61e229fb134e8d955f6c7567702dc5dddde7c9458e1f1839eca4ae793ab0592a"
    );
    assert_eq!(
        decode_installation_record(&predecessor_payload),
        Err(InstallationError::UnsupportedFormatMarker(95))
    );
    assert_eq!(
        independent_fingerprint(&bytes),
        "667970dccdc6b79e765f36c3e4267ea638f524528c8cc04e1b30326cca355bc8"
    );
    assert_eq!(
        installation_fingerprint(&record)
            .expect("installation fingerprint")
            .to_string(),
        "667970dccdc6b79e765f36c3e4267ea638f524528c8cc04e1b30326cca355bc8"
    );
    // Format 82 adds an explicit continuation count to every function row,
    // including these empty rosters. Changing only the header is not a
    // conversion back to the previous wire format.
    assert!(
        record
            .functions()
            .iter()
            .all(|function| function.unit_continuations.is_empty())
    );
    let mut previous_bytes = bytes.clone();
    previous_bytes[8..10].copy_from_slice(&81_u16.to_le_bytes());
    assert_eq!(
        decode_installation_record(&previous_bytes),
        Err(InstallationError::UnsupportedFormatMarker(81))
    );

    let mut changed_plan = plan;
    changed_plan.functions[1].bytes = integer_return(8);
    let changed_artifact = build_object_artifact(&changed_plan).expect("changed artifact");
    let changed_image =
        emit_direct_executable_image(&changed_artifact, 3).expect("changed Linux image");
    assert_eq!(
        validate_installation_record(&record, &changed_image),
        Err(InstallationError::ImageBindingMismatch)
    );
    assert!(matches!(
        derive_installation_stack_demand(&record, &changed_image, machine_id(2)),
        Err(image_emission::InstallationStackError::Installation(
            InstallationError::ImageBindingMismatch
        ))
    ));
}

/// A record seals only the placed-inventory identities: replay must prove the
/// image's retained rows still classify every final byte, leave nothing
/// unclassified, and stay consistent with the claimed section layout. Drifting
/// the image's retained inventory after the record was built rejects as
/// placement-custody failure — before any record-field comparison.
#[test]
fn installation_record_rejects_drifted_complete_image_placement() {
    let plan = internal_call_plan(NativeTarget::linux_x64());
    let artifact = build_object_artifact(&plan).expect("artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("Linux image");
    let record = build_installation_record(&image, ProfileDecisionId::new(11).expect("profile"))
        .expect("installation record");
    validate_installation_record(&record, &image).expect("exact image binding");

    // Dropping a classified executable region leaves final text bytes the
    // sealed inventory no longer accounts for.
    let mut missing = image.clone();
    missing
        .output_mut_for_test()
        .executable_regions
        .regions
        .pop();
    assert_eq!(
        validate_installation_record(&record, &missing),
        Err(InstallationError::InvalidImagePlacementCustody)
    );

    // A substituted placed address can no longer replay against the region's
    // own section offset.
    let mut substituted = image.clone();
    substituted.output_mut_for_test().executable_regions.regions[0].address += 0x1000;
    assert_eq!(
        validate_installation_record(&record, &substituted),
        Err(InstallationError::InvalidImagePlacementCustody)
    );

    // The initialized-data inventory is sealed the same way: an inflated
    // claimed extent cannot replay over the exact final data bytes.
    let mut inflated = image.clone();
    inflated.output_mut_for_test().data_regions.data_byte_count += 8;
    assert_eq!(
        validate_installation_record(&record, &inflated),
        Err(InstallationError::InvalidImagePlacementCustody)
    );

    // The Mach-O lane runs the same custody gate plus thunk↔binding-slot
    // pairing; this image has no imports, so pairing is vacuous and region
    // custody alone must reject the drift.
    let macho_plan = internal_call_plan(NativeTarget::macos_arm64());
    let macho_artifact = build_object_artifact(&macho_plan).expect("Mach-O artifact");
    let macho_image = emit_direct_executable_image(&macho_artifact, 3).expect("Mach-O image");
    let macho_record =
        build_installation_record(&macho_image, ProfileDecisionId::new(13).expect("profile"))
            .expect("Mach-O installation record");
    validate_installation_record(&macho_record, &macho_image).expect("exact Mach-O binding");
    let mut macho_drifted = macho_image.clone();
    macho_drifted
        .output_mut_for_test()
        .executable_regions
        .regions
        .pop();
    assert_eq!(
        validate_installation_record(&macho_record, &macho_drifted),
        Err(InstallationError::InvalidImagePlacementCustody)
    );
}

// Every representable installation-header axis — program identity, target,
// subsystem, profile decision, the committed component-progress projection,
// the bound image fingerprint and section layout, and the compiler
// text-validation receipt — is authenticated custody: a one-field
// substitution either cannot encode canonically or still encodes, recomputes
// a distinct installation fingerprint, and independent replay rejects it.
// Axes bound to the emitted image reject through `validate_installation_record`.
// The admission-owned axes — the caller-supplied profile decision and the
// component-progress identities — are not image facts, so the image join
// cannot see them; their custody is the published record identity that a
// deployment journal replays against its pinned fingerprint.
optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installation record's header custody.
    /// Image-bound axes and honestly recomputed receipt legs still encode and
    /// are rejected by independent replay; admission-owned axes are admitted
    /// by the image join and answered by the honest record's diverging
    /// custody; the remaining axes have no canonical shape and are rejected
    /// at encoding.
    enum InstallationHeaderFieldForTest {
        PsiProgramFingerprint,
        TargetArchitecture,
        Target,
        Image,
        LayoutTextAddress,
        LayoutDataAddress,
        LayoutBssAddress,
        FinalTextByteCountExtended,
        FinalDataByteCountExtended,
        ExecutableInventoryDigestSubstituted,
        DataInventoryDigestSubstituted,
        CompilerTextValidation,
        DerivationReportFingerprint,
        EncodedTextDigestRecomputed,
        FinalCompilerTextDigestRecomputed,
        RelocationEnvelopeDigestRecomputed,
        EncodedTextReportFingerprintRecomputed,
        FinalCompilerTextReportFingerprintRecomputed,
        RelocationEnvelopeReportFingerprintRecomputed,
        CheckedInstructionValidationReportFingerprintRecomputed,
        CheckedInstructionFootprintReportFingerprintRecomputed,
        TextRelocationCountRecomputed,
        CheckedInstructionValidationCountRecomputed,
        ProfileDecision,
        ComponentProgressManifest,
        ComponentProgressAcceptance,
        ComponentProgressAbsent,
        Subsystem,
        ObjectFormatMachO,
        ObjectFormatCoff,
        PointerSize,
        PointerAlignment,
        TextAddressAbsent,
        TextByteCount,
        DataByteCount,
        FinalDataFingerprint,
        FinalTextByteCountContracted,
        ExecutableInventoryDigestAbsent,
        DataInventoryDigestAbsent,
        EncodedTextDigestStale,
        FinalCompilerTextDigestStale,
        RelocationEnvelopeDigestStale,
        DerivationDigestStale,
        EncodedTextReportFingerprintStale,
        FinalCompilerTextReportFingerprintStale,
        RelocationEnvelopeReportFingerprintStale,
        CheckedInstructionValidationReportFingerprintStale,
        CheckedInstructionFootprintReportFingerprintStale,
        TextRelocationCountStale,
        CheckedInstructionValidationCountStale,
    }
}

/// An honestly produced installation record over the two-function Linux
/// image, retaining the committed component-progress identities the admission
/// custody legs substitute.
fn honest_installation_header_record(
    image: &image_emission::ExecutableImage,
) -> InstallationRecord {
    build_installation_record_with_evidence(
        image,
        ProfileDecisionId::new(11).expect("profile decision"),
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        Some(&TestComponentProgressAcceptance {
            manifest: 0x1122,
            acceptance: 0x3344,
        }),
    )
    .expect("installation record")
}

/// A relocation-bearing sibling record: its image fingerprint and
/// text-validation receipt supply the well-typed foreign values the
/// header-bound legs substitute.
fn foreign_header_record() -> InstallationRecord {
    let mut other_plan = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut other_plan);
    let other_artifact = build_object_artifact(&other_plan).expect("other artifact");
    let other_image = emit_direct_executable_image(&other_artifact, 3).expect("other image");
    build_installation_record(&other_image, ProfileDecisionId::new(7).expect("profile"))
        .expect("other record")
}

/// Mutate exactly one header axis. Foreign values come from `donor` where
/// they are well-typed; the component-progress legs take the staged
/// one-axis progress values captured by the test. The recomputed receipt
/// legs refresh `derivation_digest` honestly so encoding accepts a
/// self-consistent receipt and replay still rejects it; the stale legs leave
/// the digest alone.
fn substitute_installation_header(
    record: &mut InstallationRecord,
    field: InstallationHeaderFieldForTest,
    donor: &InstallationRecord,
    manifest_only: image_emission::InstalledComponentProgress,
    acceptance_only: image_emission::InstalledComponentProgress,
) {
    use InstallationHeaderFieldForTest as Field;
    match field {
        Field::PsiProgramFingerprint => {
            record.psi_mut_for_test().program_fingerprint =
                SemanticFingerprint::from_bytes([0xa5; 32]);
        }
        Field::TargetArchitecture => {
            record.target_mut_for_test().architecture = target::Architecture::Aarch64;
        }
        Field::Target => {
            *record.target_mut_for_test() = NativeTarget::macos_arm64();
        }
        Field::Image => {
            *record.image_mut_for_test() = donor.image();
        }
        Field::LayoutTextAddress => {
            record.image_sections_mut_for_test().layout.text_address += 0x1000;
        }
        Field::LayoutDataAddress => {
            record.image_sections_mut_for_test().layout.data_address += 0x1000;
        }
        Field::LayoutBssAddress => {
            record.image_sections_mut_for_test().layout.bss_address += 0x1000;
        }
        // The complete-custody extents and sealed inventory digests remain
        // representable above their floor; only the image join exposes drift.
        Field::FinalTextByteCountExtended => {
            record.image_sections_mut_for_test().final_text_byte_count += 8;
        }
        Field::FinalDataByteCountExtended => {
            record.image_sections_mut_for_test().final_data_byte_count += 8;
        }
        Field::ExecutableInventoryDigestSubstituted => {
            record
                .image_sections_mut_for_test()
                .executable_inventory_digest =
                image::PlacedExecutableRegionInventoryDigest::from_digest([0x55; 32]);
        }
        Field::DataInventoryDigestSubstituted => {
            record.image_sections_mut_for_test().data_inventory_digest =
                image::PlacedDataRegionInventoryDigest::from_digest([0x55; 32]);
        }
        Field::CompilerTextValidation => {
            *record.compiler_text_validation_mut_for_test() = donor.compiler_text_validation();
        }
        // `derivation_report_fingerprint` is report compatibility only: it is
        // outside the derivation-digest join, so the substitution is
        // representable without recomputing the receipt's own identity.
        Field::DerivationReportFingerprint => {
            record
                .compiler_text_validation_mut_for_test()
                .derivation_report_fingerprint += 1;
        }
        // Every receipt input joined by the derivation digest remains
        // representable once the containing identity is honestly recomputed:
        // encoding accepts the consistent receipt and replay still rejects it
        // against the unchanged image.
        Field::EncodedTextDigestRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.encoded_text_digest = donor.compiler_text_validation().encoded_text_digest;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        Field::FinalCompilerTextDigestRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.final_compiler_text_digest =
                donor.compiler_text_validation().final_compiler_text_digest;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        Field::RelocationEnvelopeDigestRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.relocation_envelope_digest =
                donor.compiler_text_validation().relocation_envelope_digest;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        Field::EncodedTextReportFingerprintRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.encoded_text_report_fingerprint += 1;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        Field::FinalCompilerTextReportFingerprintRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.final_compiler_text_report_fingerprint += 1;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        Field::RelocationEnvelopeReportFingerprintRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.relocation_envelope_report_fingerprint += 1;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        Field::CheckedInstructionValidationReportFingerprintRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.checked_instruction_validation_report_fingerprint += 1;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        Field::CheckedInstructionFootprintReportFingerprintRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.checked_instruction_footprint_report_fingerprint += 1;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        Field::TextRelocationCountRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.text_relocation_count += 1;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        Field::CheckedInstructionValidationCountRecomputed => {
            let evidence = record.compiler_text_validation_mut_for_test();
            evidence.checked_instruction_validation_count += 1;
            evidence.derivation_digest = evidence.recomputed_derivation_digest();
        }
        // Admission-owned axes are not image facts: the image join cannot see
        // them, so the substitution only breaks the published record
        // identity — the recomputed-fingerprint mismatch a deployment journal
        // applies.
        Field::ProfileDecision => {
            *record.profile_decision_mut_for_test() =
                ProfileDecisionId::new(12).expect("profile decision");
        }
        Field::ComponentProgressManifest => {
            *record.component_progress_mut_for_test() = Some(manifest_only);
        }
        Field::ComponentProgressAcceptance => {
            *record.component_progress_mut_for_test() = Some(acceptance_only);
        }
        Field::ComponentProgressAbsent => {
            *record.component_progress_mut_for_test() = donor.component_progress();
        }
        // ELF and Mach-O records carry no subsystem fact.
        Field::Subsystem => {
            *record.subsystem_mut_for_test() = Some(3);
        }
        // `object_format` has no canonical substitution on an x86-64 record:
        // COFF requires a subsystem, and a format the image does not carry is
        // answered by the image join.
        Field::ObjectFormatMachO => {
            record.target_mut_for_test().object_format = target::ObjectFormat::MachO;
        }
        Field::ObjectFormatCoff => {
            record.target_mut_for_test().object_format = target::ObjectFormat::Coff;
        }
        Field::PointerSize => {
            record.target_mut_for_test().pointer_size = 4;
        }
        Field::PointerAlignment => {
            record.target_mut_for_test().pointer_alignment = 16;
        }
        // The section projection re-derives its facts from the retained
        // function roster and the initialized-data prefix.
        Field::TextAddressAbsent => {
            record.image_sections_mut_for_test().layout.text_address = 0;
        }
        Field::TextByteCount => {
            record.image_sections_mut_for_test().text_byte_count += 8;
        }
        Field::DataByteCount => {
            record.image_sections_mut_for_test().data_byte_count += 8;
        }
        Field::FinalDataFingerprint => {
            record.image_sections_mut_for_test().final_data_fingerprint =
                image_emission::InitializedDataFingerprint::for_test([0x33; 32]);
        }
        // The complete final extents can never contract below the
        // compiler-authored spans they contain, and neither sealed inventory
        // digest may be absent.
        Field::FinalTextByteCountContracted => {
            record.image_sections_mut_for_test().final_text_byte_count -= 1;
        }
        Field::ExecutableInventoryDigestAbsent => {
            record
                .image_sections_mut_for_test()
                .executable_inventory_digest =
                image::PlacedExecutableRegionInventoryDigest::from_digest([0; 32]);
        }
        Field::DataInventoryDigestAbsent => {
            record.image_sections_mut_for_test().data_inventory_digest =
                image::PlacedDataRegionInventoryDigest::from_digest([0; 32]);
        }
        // Every receipt input bound by the derivation digest — and the digest
        // itself — is non-canonical without a consistent recomputation.
        Field::EncodedTextDigestStale => {
            record
                .compiler_text_validation_mut_for_test()
                .encoded_text_digest = donor.compiler_text_validation().encoded_text_digest;
        }
        Field::FinalCompilerTextDigestStale => {
            record
                .compiler_text_validation_mut_for_test()
                .final_compiler_text_digest =
                donor.compiler_text_validation().final_compiler_text_digest;
        }
        Field::RelocationEnvelopeDigestStale => {
            record
                .compiler_text_validation_mut_for_test()
                .relocation_envelope_digest =
                donor.compiler_text_validation().relocation_envelope_digest;
        }
        Field::DerivationDigestStale => {
            record
                .compiler_text_validation_mut_for_test()
                .derivation_digest = donor.compiler_text_validation().derivation_digest;
        }
        Field::EncodedTextReportFingerprintStale => {
            record
                .compiler_text_validation_mut_for_test()
                .encoded_text_report_fingerprint += 1;
        }
        Field::FinalCompilerTextReportFingerprintStale => {
            record
                .compiler_text_validation_mut_for_test()
                .final_compiler_text_report_fingerprint += 1;
        }
        Field::RelocationEnvelopeReportFingerprintStale => {
            record
                .compiler_text_validation_mut_for_test()
                .relocation_envelope_report_fingerprint += 1;
        }
        Field::CheckedInstructionValidationReportFingerprintStale => {
            record
                .compiler_text_validation_mut_for_test()
                .checked_instruction_validation_report_fingerprint += 1;
        }
        Field::CheckedInstructionFootprintReportFingerprintStale => {
            record
                .compiler_text_validation_mut_for_test()
                .checked_instruction_footprint_report_fingerprint += 1;
        }
        Field::TextRelocationCountStale => {
            record
                .compiler_text_validation_mut_for_test()
                .text_relocation_count += 1;
        }
        Field::CheckedInstructionValidationCountStale => {
            record
                .compiler_text_validation_mut_for_test()
                .checked_instruction_validation_count += 1;
        }
    }
}

/// The expected checker outcome per field: image-bound and recomputed legs
/// are rejected by replay, admission-owned legs are answered by the honest
/// record's diverging custody, and the remaining legs surface the codec's
/// rejection.
fn installation_header_outcome(
    field: InstallationHeaderFieldForTest,
) -> MutationOutcome<InstallationError> {
    use InstallationHeaderFieldForTest as Field;
    match field {
        Field::ProfileDecision
        | Field::ComponentProgressManifest
        | Field::ComponentProgressAcceptance
        | Field::ComponentProgressAbsent => MutationOutcome::RebuiltCustodyDiffers,
        Field::Subsystem => MutationOutcome::ExactError(InstallationError::UnexpectedSubsystem),
        Field::ObjectFormatMachO => {
            MutationOutcome::ExactError(InstallationError::ImageBindingMismatch)
        }
        Field::ObjectFormatCoff => {
            MutationOutcome::ExactError(InstallationError::MissingCoffSubsystem)
        }
        Field::PointerSize => {
            MutationOutcome::ExactError(InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::Elf,
                pointer_size: 4,
                pointer_alignment: 8,
            }))
        }
        Field::PointerAlignment => {
            MutationOutcome::ExactError(InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::Elf,
                pointer_size: 8,
                pointer_alignment: 16,
            }))
        }
        Field::TextAddressAbsent
        | Field::TextByteCount
        | Field::DataByteCount
        | Field::FinalDataFingerprint
        | Field::FinalTextByteCountContracted
        | Field::ExecutableInventoryDigestAbsent
        | Field::DataInventoryDigestAbsent => {
            MutationOutcome::ExactError(InstallationError::InvalidImageSectionLayout)
        }
        Field::EncodedTextDigestStale
        | Field::FinalCompilerTextDigestStale
        | Field::RelocationEnvelopeDigestStale
        | Field::DerivationDigestStale
        | Field::EncodedTextReportFingerprintStale
        | Field::FinalCompilerTextReportFingerprintStale
        | Field::RelocationEnvelopeReportFingerprintStale
        | Field::CheckedInstructionValidationReportFingerprintStale
        | Field::CheckedInstructionFootprintReportFingerprintStale
        | Field::TextRelocationCountStale
        | Field::CheckedInstructionValidationCountStale => {
            MutationOutcome::ExactError(InstallationError::InvalidCompilerTextDerivationDigest)
        }
        _ => MutationOutcome::ExactError(InstallationError::ImageBindingMismatch),
    }
}

// The subsystem axis is representable only where the writer records it: on a
// PE/COFF image the substituted subsystem still encodes and replay against
// the unchanged image rejects it; an absent subsystem, a non-COFF target,
// and a format the image does not carry are rejected at canonical encoding
// or the image join.
optimization_core::custody_field_inventory! {
    /// One substitutable axis of a PE/COFF installation record's header
    /// custody: the carried subsystem is replay-bound, the rest are
    /// canonical-shape failures.
    enum CoffInstallationHeaderFieldForTest {
        SubsystemSubstituted,
        SubsystemAbsent,
        TargetSubstituted,
        ObjectFormatMachO,
    }
}

/// An honestly produced installation record over the two-function PE image:
/// COFF is the only target family whose records carry a subsystem.
fn honest_coff_installation_record() -> InstallationRecord {
    let mut coff_plan = two_function_plan();
    coff_plan.target = NativeTarget::windows_x64();
    let coff_artifact = build_object_artifact(&coff_plan).expect("COFF artifact");
    let coff_image = emit_direct_executable_image(&coff_artifact, 3).expect("PE image");
    build_installation_record(&coff_image, ProfileDecisionId::new(19).expect("profile"))
        .expect("COFF record")
}

/// Mutate exactly one COFF header axis.
fn substitute_coff_installation_header(
    record: &mut InstallationRecord,
    field: CoffInstallationHeaderFieldForTest,
    _donor: &InstallationRecord,
) {
    use CoffInstallationHeaderFieldForTest as Field;
    match field {
        Field::SubsystemSubstituted => {
            *record.subsystem_mut_for_test() = Some(4);
        }
        Field::SubsystemAbsent => {
            *record.subsystem_mut_for_test() = None;
        }
        Field::TargetSubstituted => {
            *record.target_mut_for_test() = NativeTarget::linux_x64();
        }
        Field::ObjectFormatMachO => {
            record.target_mut_for_test().object_format = target::ObjectFormat::MachO;
        }
    }
}

/// The expected checker outcome per COFF field.
fn coff_installation_header_outcome(
    field: CoffInstallationHeaderFieldForTest,
) -> MutationOutcome<InstallationError> {
    use CoffInstallationHeaderFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::SubsystemSubstituted => InstallationError::ImageBindingMismatch,
        Field::SubsystemAbsent => InstallationError::MissingCoffSubsystem,
        // A retained subsystem is non-canonical on every non-COFF target.
        Field::TargetSubstituted | Field::ObjectFormatMachO => {
            InstallationError::UnexpectedSubsystem
        }
    })
}

#[test]
fn installation_header_rejects_every_one_field_substitution() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("Linux image");
    let record = honest_installation_header_record(&image);
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_evidence = record.compiler_text_validation();
    let authentic_progress = record
        .component_progress()
        .expect("committed component progress");
    assert_eq!(authentic_progress.manifest_identity(), 0x1122);
    assert_eq!(authentic_progress.acceptance_identity(), 0x3344);
    assert_eq!(record.subsystem(), None);

    // A relocation-bearing sibling image supplies well-typed foreign values
    // for the bound image fingerprint and the receipt digest axes.
    let donor = foreign_header_record();
    let other_evidence = donor.compiler_text_validation();
    assert_ne!(donor.image(), record.image());
    assert_ne!(
        other_evidence.encoded_text_digest, authentic_evidence.encoded_text_digest,
        "sibling text differs"
    );
    assert_ne!(
        other_evidence.final_compiler_text_digest, authentic_evidence.final_compiler_text_digest,
        "sibling final text differs"
    );
    assert_ne!(
        other_evidence.relocation_envelope_digest, authentic_evidence.relocation_envelope_digest,
        "a retained relocation changes the envelope commitment"
    );
    assert_ne!(
        other_evidence.derivation_digest, authentic_evidence.derivation_digest,
        "sibling derivation identity differs"
    );

    // Component-progress projections differing in exactly one committed
    // identity.
    let manifest_only = build_installation_record_with_evidence(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        Some(&TestComponentProgressAcceptance {
            manifest: 0x5566,
            acceptance: 0x3344,
        }),
    )
    .expect("manifest-substituted record")
    .component_progress()
    .expect("manifest progress");
    let acceptance_only = build_installation_record_with_evidence(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        Some(&TestComponentProgressAcceptance {
            manifest: 0x1122,
            acceptance: 0x7788,
        }),
    )
    .expect("acceptance-substituted record")
    .component_progress()
    .expect("acceptance progress");
    assert_ne!(Some(manifest_only), record.component_progress());
    assert_ne!(Some(acceptance_only), record.component_progress());

    let substitute = |record: &mut InstallationRecord,
                      field: InstallationHeaderFieldForTest,
                      donor: &InstallationRecord| {
        substitute_installation_header(record, field, donor, manifest_only, acceptance_only)
    };
    // Encode/decode preserves a substitution; the image join re-binds every
    // image-visible axis, and an accepted admission-owned substitution is
    // answered by the honest record the published identity replays.
    let check = |record: &InstallationRecord| -> Result<InstallationRecord, InstallationError> {
        let bytes = encode_installation_record(record)?;
        let replayed = decode_installation_record(&bytes)?;
        assert_eq!(&replayed, record, "codec preserves the substituted record");
        validate_installation_record(&replayed, &image)?;
        Ok(honest_installation_header_record(&image))
    };
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation record header",
        fields: InstallationHeaderFieldForTest::INVENTORY,
        honest: &|| honest_installation_header_record(&image),
        donor: donor.clone(),
        custody: &installation_record_custody,
        substitute: &substitute,
        check: &check,
        outcome: &installation_header_outcome,
        joined_replay: None,
    });

    // The subsystem axis is representable only where the writer records it:
    // on a PE/COFF image the substituted subsystem still encodes and replay
    // against the unchanged image rejects it.
    let mut coff_plan = two_function_plan();
    coff_plan.target = NativeTarget::windows_x64();
    let coff_artifact = build_object_artifact(&coff_plan).expect("COFF artifact");
    let coff_image = emit_direct_executable_image(&coff_artifact, 3).expect("PE image");
    let coff_record = honest_coff_installation_record();
    assert_eq!(coff_record.subsystem(), Some(3));
    validate_installation_record(&coff_record, &coff_image).expect("COFF binding");

    let coff_check =
        |record: &InstallationRecord| -> Result<InstallationRecord, InstallationError> {
            let bytes = encode_installation_record(record)?;
            let replayed = decode_installation_record(&bytes)?;
            assert_eq!(&replayed, record, "codec preserves the substituted record");
            validate_installation_record(&replayed, &coff_image)?;
            Ok(honest_coff_installation_record())
        };
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "COFF installation record header",
        fields: CoffInstallationHeaderFieldForTest::INVENTORY,
        honest: &honest_coff_installation_record,
        donor: donor.clone(),
        custody: &installation_record_custody,
        substitute: &substitute_coff_installation_header,
        check: &coff_check,
        outcome: &coff_installation_header_outcome,
        joined_replay: None,
    });

    // Axes without an in-memory representation still reject at the wire: the
    // fixed magic and format marker, the single unstable vocabulary marker,
    // unknown enum tags, the reserved field, zero profile and committed
    // progress identities, and presence-flag lies are alternate or malformed
    // encodings, never a canonical record.
    let canonical = encode_installation_record(&record).expect("canonical bytes");
    let mut foreign_magic = canonical.clone();
    foreign_magic[0] ^= 0xff;
    assert_eq!(
        decode_installation_record(&foreign_magic),
        Err(InstallationError::InvalidMagic)
    );
    let mut foreign_format = canonical.clone();
    foreign_format[8..10].copy_from_slice(&(INSTALLATION_FORMAT_MARKER + 1).to_le_bytes());
    assert_eq!(
        decode_installation_record(&foreign_format),
        Err(InstallationError::UnsupportedFormatMarker(
            INSTALLATION_FORMAT_MARKER + 1
        ))
    );
    let mut foreign_marker = canonical.clone();
    foreign_marker[10..12].copy_from_slice(&(VocabularyMarker::CURRENT.get() + 1).to_le_bytes());
    assert_eq!(
        decode_installation_record(&foreign_marker),
        Err(InstallationError::UnsupportedVocabularyMarker(
            VocabularyMarker::CURRENT.get() + 1
        ))
    );
    let mut foreign_architecture = canonical.clone();
    foreign_architecture[44] = 0;
    assert_eq!(
        decode_installation_record(&foreign_architecture),
        Err(InstallationError::InvalidArchitectureTag(0))
    );
    let mut foreign_object_format = canonical.clone();
    foreign_object_format[45] = 0;
    assert_eq!(
        decode_installation_record(&foreign_object_format),
        Err(InstallationError::InvalidObjectFormatTag(0))
    );
    let mut nonzero_reserved = canonical.clone();
    nonzero_reserved[66] = 1;
    assert_eq!(
        decode_installation_record(&nonzero_reserved),
        Err(InstallationError::NonzeroReservedField)
    );
    let mut zero_profile = canonical.clone();
    zero_profile[68..76].fill(0);
    assert_eq!(
        decode_installation_record(&zero_profile),
        Err(InstallationError::ZeroProfileDecision)
    );
    // The committed progress identities sit at fixed header offsets behind
    // the presence flag; pinning the bytes before zeroing keeps the axes
    // aligned with the encoder layout.
    assert_eq!(&canonical[76..84], &0x1122_u64.to_le_bytes());
    assert_eq!(&canonical[84..92], &0x3344_u64.to_le_bytes());
    let mut zero_manifest = canonical.clone();
    zero_manifest[76..84].fill(0);
    assert_eq!(
        decode_installation_record(&zero_manifest),
        Err(InstallationError::ZeroComponentProgressManifestIdentity)
    );
    let mut zero_acceptance = canonical.clone();
    zero_acceptance[84..92].fill(0);
    assert_eq!(
        decode_installation_record(&zero_acceptance),
        Err(InstallationError::ZeroComponentProgressAcceptanceIdentity)
    );
    let mut hidden_subsystem = canonical.clone();
    hidden_subsystem[64..66].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        decode_installation_record(&hidden_subsystem),
        Err(InstallationError::NonCanonicalSubsystem)
    );
    // A presence flag without its payload decodes an out-of-shape record.
    let mut claimed_subsystem = canonical.clone();
    claimed_subsystem[46] = 1;
    assert_eq!(
        decode_installation_record(&claimed_subsystem),
        Err(InstallationError::UnexpectedSubsystem)
    );
    let mut dropped_progress = canonical.clone();
    dropped_progress[47] = 0;
    assert_eq!(
        decode_installation_record(&dropped_progress),
        Err(InstallationError::InvalidCallSiteOwnerTag(0)),
        "an absent progress flag cannot absorb the committed identity bytes"
    );
}

// Every representable axis of an installed function row is an authenticated
// custody axis: a one-field substitution either cannot encode canonically or
// still encodes, recomputes a distinct installation fingerprint, and
// independent replay against the unchanged image rejects it. Roster-level
// rows (call stacks, homes, continuations) are covered by their producing
// fixtures elsewhere.
optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installed-function row inside a retained
    /// [`image_emission::InstallationRecord`]. The machine, attachment and
    /// stack legs still encode canonically and are rejected by independent
    /// replay; text-interval geometry, a dropped row and the `unit_body`
    /// projection have no canonical shape join and are rejected at encoding.
    /// The shared `run_one_field_substitution_matrix` driver iterates this
    /// inventory, so a new representable field is one more variant.
    enum InstalledFunctionRowFieldForTest {
        Machine,
        Attachment,
        UnitStack,
        ScalarStack,
        TextOffsetShift,
        ByteCountShortened,
        RowDropped,
        UnitBody,
    }
}

/// An honestly produced two-function installation record: the function-row
/// matrix's staging is deterministic, so each leg re-derives the identical
/// record rather than cloning retained state.
fn honest_installed_function_record() -> InstallationRecord {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("Linux image");
    build_installation_record(&image, ProfileDecisionId::new(11).expect("profile"))
        .expect("installation record")
}

/// Mutate exactly one axis of the second retained function row. Fixed
/// alternates cannot equal any honest value; `donor` grants no new authority
/// and is retained for signature uniformity.
fn substitute_installed_function_row(
    record: &mut InstallationRecord,
    field: InstalledFunctionRowFieldForTest,
    _donor: &InstallationRecord,
) {
    use InstalledFunctionRowFieldForTest as Field;
    match field {
        Field::RowDropped => {
            record.functions_mut_for_test().pop();
        }
        Field::Machine => {
            let current = record.functions()[1].machine;
            record.functions_mut_for_test()[1].machine =
                MachineId::new(current.get() + 100).expect("drifted machine");
        }
        Field::Attachment => {
            record.functions_mut_for_test()[1].attachment =
                Some(StructuralTypeId::new(7).expect("drifted attachment"));
        }
        Field::UnitStack => {
            record.functions_mut_for_test()[1].unit_stack = Some(image_emission::ObjectUnitStack {
                frame_bytes: 16,
                local_peak_bytes: 0,
                stack_alignment: 16,
            });
        }
        Field::ScalarStack => {
            record.functions_mut_for_test()[1].scalar_stack =
                Some(image_emission::ObjectScalarStack {
                    local_peak_bytes: 0,
                    stack_alignment: 16,
                });
        }
        // Text intervals are not independently representable: canonical rows
        // are contiguous and exhaust the text section, so a shifted offset or
        // shortened interval fails the canonical-shape join at encoding.
        Field::TextOffsetShift => {
            record.functions_mut_for_test()[1].text_offset += 1;
        }
        Field::ByteCountShortened => {
            record.functions_mut_for_test()[1].byte_count -= 1;
        }
        // `unit_body` is a projection of the retained affine cleanup; flipping
        // it alone is likewise rejected at encoding.
        Field::UnitBody => {
            record.functions_mut_for_test()[1].unit_body = true;
        }
    }
}

/// The expected checker outcome per field: replay-bound axes surface as
/// `ImageBindingMismatch` through encode/decode/replay, canonical-shape axes
/// surface as the codec's rejection.
fn installed_function_row_outcome(
    field: InstalledFunctionRowFieldForTest,
) -> MutationOutcome<InstallationError> {
    use InstalledFunctionRowFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::Machine | Field::Attachment | Field::UnitStack | Field::ScalarStack => {
            InstallationError::ImageBindingMismatch
        }
        Field::TextOffsetShift => InstallationError::NonCanonicalInstalledFunctions,
        Field::ByteCountShortened | Field::RowDropped => {
            InstallationError::InvalidImageSectionLayout
        }
        Field::UnitBody => InstallationError::InvalidUnitAffineCleanup(machine_id(2)),
    })
}

/// Every representable field of an installed-function row is an authenticated
/// custody axis: a one-field substitution either cannot encode canonically or
/// still encodes, recomputes a distinct installation fingerprint, and
/// independent replay against the unchanged image rejects it.
#[test]
fn installation_function_row_rejects_every_one_field_substitution() {
    let image = emit_direct_executable_image(
        &build_object_artifact(&two_function_plan()).expect("artifact"),
        3,
    )
    .expect("Linux image");
    let record = honest_installed_function_record();
    validate_installation_record(&record, &image).expect("exact image binding");
    let [_, authentic] = record.functions() else {
        panic!("two-function fixture retains two rows");
    };
    assert_eq!(authentic.attachment, None);
    assert!(!authentic.unit_body);
    assert_eq!(authentic.unit_stack, None);
    assert_eq!(authentic.scalar_stack, None);

    let check = installation_record_check(&image);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installed function row",
        fields: InstalledFunctionRowFieldForTest::INVENTORY,
        honest: &honest_installed_function_record,
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_installed_function_row,
        check: &check,
        outcome: &installed_function_row_outcome,
        joined_replay: None,
    });
}

// Every representable field of an installed internal Unit-call row is an
// authenticated custody axis: a one-field substitution either cannot encode
// canonically or still encodes, recomputes a distinct installation
// fingerprint, and independent replay against the unchanged image rejects
// it.
optimization_core::custody_field_inventory! {
    /// One substitutable axis of the installed internal Unit-call roster.
    /// The cleanup-owned call's `byte_count` is the one field no record-shape
    /// join pins — the cleanup action's extent is carried by the cleanup
    /// record itself — so that leg still encodes and is rejected by
    /// independent replay. Every other field on every row is a canonical
    /// projection bound by the caller's attribution and cleanup joins or the
    /// callee's own call shape, so those substitutions are rejected at
    /// encoding.
    enum InstalledInternalUnitCallFieldForTest {
        ByteCountCleanup,
        Machine,
        TextOffset,
        Owner,
        OwnerCleanup,
        Source,
        Target,
        Result,
        StructuralResult,
        ScalarArguments,
        Arguments,
        ClaimTransfers,
        OperationOrdinal,
        CodeOffset,
        CodeOffsetSecondCall,
        ByteCount,
        ByteCountSecondCall,
        MachineCleanup,
        TextOffsetCleanup,
        CodeOffsetCleanup,
        OperationOrdinalCleanup,
        SourceCleanup,
        ResultCleanup,
        ScalarArgumentsCleanup,
        ArgumentsCleanup,
        StructuralResultCleanup,
        TargetSecondCall,
        OperationOrdinalSecondCall,
        ScalarArgumentsSecondCall,
        TargetCleanup,
        ClaimTransfersCleanup,
        RowDropped,
        SemanticResult,
    }
}

/// An honestly produced three-call installation record over the edge-owned
/// cleanup fixture: two operation-owned calls and one cleanup-action call.
fn honest_internal_unit_call_record() -> InstallationRecord {
    let artifact =
        build_object_artifact(&two_call_edge_owned_cleanup_plan()).expect("cleanup artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("cleanup image");
    build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("cleanup installation")
}

/// A foreign installed-provider call source for the `source` substitutions.
fn foreign_internal_unit_call_source() -> machine_code::InternalUnitCallSource {
    machine_code::InternalUnitCallSource::InstalledProvider {
        boundary: semantic_vocabulary::BoundaryMachineId::new(7).unwrap(),
        provider: Box::new(terminal_psi::ProviderCandidateConformance {
            boundary: semantic_vocabulary::BoundaryMachineId::new(7).unwrap(),
            requirement_identity: "requirement".into(),
            provider_identity: "provider".into(),
            candidate_identity: "candidate".into(),
            candidate: machine_id(4),
            signature: terminal_psi::ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: terminal_psi::ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        }),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    }
}

/// A foreign affine structural-result shape for the `structural_result`
/// substitutions.
fn foreign_affine_structural_result() -> machine_code::InternalStructuralCallResult {
    let empty_placement = ValuePlacement {
        shape: ValueShape::integer(0, 1),
        locations: Vec::new(),
    };
    machine_code::InternalStructuralCallResult {
        operation_result: terminal_psi::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: PlaceId::new(31).unwrap(),
            structural_type: StructuralTypeId::new(31).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        result_home: None,
        function_result: terminal_psi::StructuralResultDeclaration {
            place: PlaceId::new(31).unwrap(),
            structural_type: StructuralTypeId::new(31).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        },
        returned_claim_transfers: Vec::new(),
        returned_claims: Vec::new(),
        caller_result_placement: empty_placement.clone(),
        callee_result_placement: empty_placement,
    }
}

/// A foreign scalar-argument record for the `scalar_arguments` legs.
fn foreign_scalar_call_argument() -> machine_code::InternalUnitScalarCallArgumentRecord {
    machine_code::InternalUnitScalarCallArgumentRecord {
        parameter_index: 0,
        source: machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation: operation_id(1),
            source_value: semantic_vocabulary::ValueId::new(31).unwrap(),
            scalar_type: semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Signed,
                32,
            )
            .expect("i32"),
            value: semantic_vocabulary::IntegerValue::Signed(7),
        },
        destination: ValuePlacement {
            shape: ValueShape::integer(4, 4),
            locations: Vec::new(),
        },
        code_offset: 0,
        byte_count: 4,
    }
}

/// A foreign structural-argument record for the `arguments` legs.
fn foreign_structural_call_argument() -> machine_code::InternalUnitCallArgumentRecord {
    let empty_placement = ValuePlacement {
        shape: ValueShape::integer(0, 1),
        locations: Vec::new(),
    };
    machine_code::InternalUnitCallArgumentRecord {
        place: PlaceId::new(31).unwrap(),
        access: StructuralAccess::Owned,
        path: Vec::new(),
        root_structural_type: StructuralTypeId::new(31).unwrap(),
        structural_type: StructuralTypeId::new(31).unwrap(),
        shape: ValueShape::integer(0, 1),
        source_byte_offset: 0,
        source_location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
        call_stack_bytes: 0,
        fixed_array_length: None,
        element_stride: None,
        source: machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
            empty_placement.clone(),
        ),
        destination: empty_placement,
        code_offset: 0,
        byte_count: 0,
        bytes: Vec::new(),
    }
}

/// Mutate exactly one internal Unit-call axis; row-owned legs index into the
/// first operation call (`0`), the second operation call (`1`), or the
/// cleanup-action call (`2`). `donor` grants no new authority and is retained
/// for signature uniformity.
fn substitute_internal_unit_call(
    record: &mut InstallationRecord,
    field: InstalledInternalUnitCallFieldForTest,
    _donor: &InstallationRecord,
) {
    use InstalledInternalUnitCallFieldForTest as Field;
    match field {
        Field::ByteCountCleanup => {
            record.internal_unit_calls_mut_for_test()[2]
                .custody
                .byte_count += 1;
        }
        Field::RowDropped => {
            record.internal_unit_calls_mut_for_test().remove(0);
        }
        Field::Machine => {
            record.internal_unit_calls_mut_for_test()[0].machine = machine_id(2);
        }
        Field::TextOffset => {
            record.internal_unit_calls_mut_for_test()[0].text_offset += 1;
        }
        Field::Owner => {
            record.internal_unit_calls_mut_for_test()[0].custody.owner =
                CallSiteOwner::Operation(operation_id(7));
        }
        Field::OwnerCleanup => {
            record.internal_unit_calls_mut_for_test()[2].custody.owner =
                CallSiteOwner::CleanupAction {
                    edge: edge_id(4),
                    action_ordinal: 1,
                };
        }
        Field::Source => {
            record.internal_unit_calls_mut_for_test()[0].custody.source =
                foreign_internal_unit_call_source();
        }
        Field::Target => {
            record.internal_unit_calls_mut_for_test()[0].custody.target = machine_id(4);
        }
        Field::Result => {
            record.internal_unit_calls_mut_for_test()[0].custody.result =
                Some(semantic_vocabulary::ScalarType::Boolean);
        }
        Field::StructuralResult => {
            record.internal_unit_calls_mut_for_test()[0]
                .custody
                .structural_result = Some(foreign_affine_structural_result());
        }
        Field::ScalarArguments => {
            record.internal_unit_calls_mut_for_test()[0]
                .custody
                .scalar_arguments
                .push(foreign_scalar_call_argument());
        }
        Field::Arguments => {
            record.internal_unit_calls_mut_for_test()[0]
                .custody
                .arguments
                .push(foreign_structural_call_argument());
        }
        Field::ClaimTransfers => {
            record.internal_unit_calls_mut_for_test()[0]
                .custody
                .claim_transfers
                .push(terminal_psi::ClaimTransfer {
                    claim: ClaimId::new(31).unwrap(),
                    argument_index: 0,
                });
        }
        Field::OperationOrdinal => {
            record.internal_unit_calls_mut_for_test()[0]
                .custody
                .operation_ordinal += 1;
        }
        Field::CodeOffset => {
            record.internal_unit_calls_mut_for_test()[0]
                .custody
                .code_offset += 1;
        }
        Field::CodeOffsetSecondCall => {
            record.internal_unit_calls_mut_for_test()[1]
                .custody
                .code_offset += 1;
        }
        Field::ByteCount => {
            record.internal_unit_calls_mut_for_test()[0]
                .custody
                .byte_count += 1;
        }
        Field::ByteCountSecondCall => {
            record.internal_unit_calls_mut_for_test()[1]
                .custody
                .byte_count += 1;
        }
        Field::MachineCleanup => {
            record.internal_unit_calls_mut_for_test()[2].machine = machine_id(4);
        }
        Field::TextOffsetCleanup => {
            record.internal_unit_calls_mut_for_test()[2].text_offset += 1;
        }
        Field::CodeOffsetCleanup => {
            record.internal_unit_calls_mut_for_test()[2]
                .custody
                .code_offset += 1;
        }
        Field::OperationOrdinalCleanup => {
            record.internal_unit_calls_mut_for_test()[2]
                .custody
                .operation_ordinal += 1;
        }
        Field::SourceCleanup => {
            record.internal_unit_calls_mut_for_test()[2].custody.source =
                foreign_internal_unit_call_source();
        }
        Field::ResultCleanup => {
            record.internal_unit_calls_mut_for_test()[2].custody.result =
                Some(semantic_vocabulary::ScalarType::Boolean);
        }
        Field::ScalarArgumentsCleanup => {
            record.internal_unit_calls_mut_for_test()[2]
                .custody
                .scalar_arguments
                .push(foreign_scalar_call_argument());
        }
        Field::ArgumentsCleanup => {
            record.internal_unit_calls_mut_for_test()[2]
                .custody
                .arguments
                .push(foreign_structural_call_argument());
        }
        Field::StructuralResultCleanup => {
            record.internal_unit_calls_mut_for_test()[2]
                .custody
                .structural_result = Some(foreign_affine_structural_result());
        }
        Field::TargetSecondCall => {
            record.internal_unit_calls_mut_for_test()[1].custody.target = machine_id(2);
        }
        Field::OperationOrdinalSecondCall => {
            record.internal_unit_calls_mut_for_test()[1]
                .custody
                .operation_ordinal += 1;
        }
        Field::ScalarArgumentsSecondCall => {
            record.internal_unit_calls_mut_for_test()[1]
                .custody
                .scalar_arguments
                .push(foreign_scalar_call_argument());
        }
        Field::TargetCleanup => {
            record.internal_unit_calls_mut_for_test()[2].custody.target = machine_id(4);
        }
        Field::ClaimTransfersCleanup => {
            record.internal_unit_calls_mut_for_test()[2]
                .custody
                .claim_transfers
                .push(terminal_psi::ClaimTransfer {
                    claim: ClaimId::new(31).unwrap(),
                    argument_index: 0,
                });
        }
        // `semantic_result` is a canonical projection of `result`: a value
        // whose scalar type does not equal the retained ABI result is
        // rejected at encoding rather than reaching replay.
        Field::SemanticResult => {
            record.internal_unit_calls_mut_for_test()[0]
                .custody
                .semantic_result = Some(abstract_operations::AbstractResult {
                value: semantic_vocabulary::ValueId::new(31).unwrap(),
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
            });
        }
    }
}

/// The expected checker outcome per field: the cleanup `byte_count` leg is
/// rejected by replay; every canonical-shape leg surfaces the codec's
/// rejection attributed to the join that owns it.
fn internal_unit_call_outcome(
    field: InstalledInternalUnitCallFieldForTest,
) -> MutationOutcome<InstallationError> {
    use InstalledInternalUnitCallFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::ByteCountCleanup => InstallationError::ImageBindingMismatch,
        Field::TextOffset
        | Field::Owner
        | Field::Source
        | Field::ScalarArguments
        | Field::CodeOffsetSecondCall
        | Field::ByteCountSecondCall
        | Field::ScalarArgumentsSecondCall
        | Field::SemanticResult => InstallationError::InvalidInternalUnitCall(machine_id(1)),
        Field::TextOffsetCleanup
        | Field::OperationOrdinalCleanup
        | Field::SourceCleanup
        | Field::ResultCleanup
        | Field::ScalarArgumentsCleanup => {
            InstallationError::InvalidInternalUnitCall(machine_id(3))
        }
        Field::StructuralResult => InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        _ => InstallationError::InvalidUnitAffineCleanup(machine_id(3)),
    })
}

#[test]
fn installation_internal_unit_call_row_rejects_every_one_field_substitution() {
    let artifact =
        build_object_artifact(&two_call_edge_owned_cleanup_plan()).expect("cleanup artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("cleanup image");
    let record = honest_internal_unit_call_record();
    validate_installation_record(&record, &image).expect("exact image binding");
    assert_eq!(record.internal_unit_calls().len(), 3);
    let authentic = &record.internal_unit_calls()[0];
    assert_eq!(authentic.machine, machine_id(1));
    assert_eq!(
        authentic.custody.owner,
        CallSiteOwner::Operation(operation_id(1))
    );
    assert_eq!(authentic.custody.target, machine_id(2));
    assert_eq!(authentic.custody.result, None);
    assert_eq!(authentic.custody.semantic_result, None);
    assert_eq!(authentic.custody.structural_result, None);
    assert!(authentic.custody.scalar_arguments.is_empty());
    assert!(authentic.custody.arguments.is_empty());
    assert!(authentic.custody.claim_transfers.is_empty());
    assert_eq!(
        record.internal_unit_calls()[2].custody.owner,
        CallSiteOwner::CleanupAction {
            edge: edge_id(3),
            action_ordinal: 0,
        }
    );

    let check = installation_record_check(&image);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installed internal Unit-call row",
        fields: InstalledInternalUnitCallFieldForTest::INVENTORY,
        honest: &honest_internal_unit_call_record,
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_internal_unit_call,
        check: &check,
        outcome: &internal_unit_call_outcome,
        joined_replay: None,
    });
}

#[test]
fn installation_record_fingerprints_component_progress_acceptance() {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("image");
    let profile = ProfileDecisionId::new(12).expect("profile decision");
    let plain = build_installation_record(&image, profile).expect("plain record");
    let acceptance = TestComponentProgressAcceptance {
        manifest: 0x1122,
        acceptance: 0x3344,
    };
    let committed = build_installation_record_with_evidence(
        &image,
        profile,
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        Some(&acceptance),
    )
    .expect("progress-bound record");

    let progress = committed
        .component_progress()
        .expect("component progress projection");
    assert_eq!(progress.manifest_identity(), 0x1122);
    assert_eq!(progress.acceptance_identity(), 0x3344);
    assert_ne!(
        installation_fingerprint(&plain).expect("plain fingerprint"),
        installation_fingerprint(&committed).expect("committed fingerprint")
    );
    let bytes = encode_installation_record(&committed).expect("canonical bytes");
    assert_eq!(decode_installation_record(&bytes), Ok(committed));

    let zero = TestComponentProgressAcceptance {
        manifest: 0,
        acceptance: 0x3344,
    };
    assert_eq!(
        build_installation_record_with_evidence(
            &image,
            profile,
            std::iter::empty::<&dyn ProviderExecutionEvidence>(),
            Some(&zero),
        ),
        Err(InstallationError::ZeroComponentProgressManifestIdentity)
    );
}

#[test]
fn installation_record_retains_selected_provider_plan_without_execution() {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("image");
    let profile = ProfileDecisionId::new(13).expect("profile decision");
    let record = build_installation_record_with_selected_provider_plans_and_evidence(
        &image,
        profile,
        [91],
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        None,
        boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
    )
    .expect("selected but unexecuted provider plan remains installation identity");

    assert_eq!(record.selected_provider_plans()[0].get(), 91);
    let bytes = encode_installation_record(&record).expect("canonical bytes");
    assert_eq!(decode_installation_record(&bytes), Ok(record));

    assert_eq!(
        build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            [91, 91],
            std::iter::empty::<&dyn ProviderExecutionEvidence>(),
            None,
            boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        ),
        Err(InstallationError::DuplicateProviderPlan)
    );
}

// The selected provider-plan roster is admission-owned custody. The executed
// subset is pinned by the boundary settlements' admitted-provider execution
// records: substituting or dropping the executed plan, clearing or reordering
// the roster, or duplicating an entry is rejected at canonical encoding. The
// unexecuted remainder is representable — exact selection authority stays
// outside the decodable record — so each substitution still encodes,
// recomputes a distinct installation fingerprint, and is rejected by the
// published record identity a deployment journal replays rather than by the
// image join. Malformed identities and non-canonical wire order reject at
// decode.
optimization_core::custody_field_inventory! {
    /// One substitutable axis of the selected provider-plan roster. The
    /// unexecuted-selection legs are admitted substitutions the image join
    /// cannot see — replay still accepts, so the checker returns the honest
    /// record and the driver requires its custody to differ. The executed and
    /// shape legs have no canonical encoding and fail before any identity or
    /// replay check.
    enum SelectedProviderPlanFieldForTest {
        UnexecutedElement,
        ExtendedSelection,
        DroppedUnexecuted,
        ExecutedElement,
        DroppedExecuted,
        ClearedRoster,
        ReorderedRoster,
        DuplicatedElement,
        OutOfOrderSubstitution,
    }
}

/// The honestly staged port-effect record: plan 7 executes (the settlement's
/// admitted-provider execution requires it) while plan 42 is selected but
/// never executes in this image.
fn honest_selected_provider_plan_record(
    provider: &WriteExitProvider,
    image: &image_emission::ExecutableImage,
    profile: ProfileDecisionId,
) -> InstallationRecord {
    build_installation_record_with_selected_provider_plans_and_evidence(
        image,
        profile,
        [7, 42],
        [provider],
        None,
        boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
    )
    .expect("selected closure with an unexecuted plan")
}

/// Mutate the selected provider-plan roster on exactly one axis. The
/// unexecuted legs keep the required executed plan; the encoded-shape legs
/// produce a roster no canonical encoding carries.
fn substitute_selected_provider_plan(
    record: &mut InstallationRecord,
    field: SelectedProviderPlanFieldForTest,
    _donor: &InstallationRecord,
) {
    use SelectedProviderPlanFieldForTest as Field;
    let plan_id = |identity: u64| {
        image_emission::SelectedProviderPlanReportIdentity::new(identity)
            .expect("nonzero plan identity")
    };
    match field {
        Field::UnexecutedElement => {
            *record.selected_provider_plans_mut_for_test() = vec![plan_id(7), plan_id(43)];
        }
        Field::ExtendedSelection => {
            *record.selected_provider_plans_mut_for_test() =
                vec![plan_id(7), plan_id(42), plan_id(99)];
        }
        Field::DroppedUnexecuted => {
            *record.selected_provider_plans_mut_for_test() = vec![plan_id(7)];
        }
        Field::ExecutedElement => {
            record.selected_provider_plans_mut_for_test()[0] = plan_id(8);
        }
        Field::DroppedExecuted => {
            record.selected_provider_plans_mut_for_test().remove(0);
        }
        Field::ClearedRoster => {
            record.selected_provider_plans_mut_for_test().clear();
        }
        Field::ReorderedRoster => {
            record.selected_provider_plans_mut_for_test().swap(0, 1);
        }
        Field::DuplicatedElement => {
            let plans = record.selected_provider_plans_mut_for_test();
            plans.insert(1, plans[0]);
        }
        Field::OutOfOrderSubstitution => {
            record.selected_provider_plans_mut_for_test()[1] = plan_id(3);
        }
    }
}

/// Replay admitted the roster substitution; the published deployment-journal
/// identity is the honest record's, so the checker returns the honest record
/// and the driver requires its custody to differ from the substituted one.
fn selected_provider_plan_outcome(
    field: SelectedProviderPlanFieldForTest,
) -> MutationOutcome<InstallationError> {
    use SelectedProviderPlanFieldForTest as Field;
    match field {
        Field::UnexecutedElement | Field::ExtendedSelection | Field::DroppedUnexecuted => {
            MutationOutcome::RebuiltCustodyDiffers
        }
        Field::ExecutedElement | Field::DroppedExecuted | Field::ClearedRoster => {
            MutationOutcome::ExactError(InstallationError::ProviderSettlementClosureMismatch)
        }
        Field::ReorderedRoster | Field::DuplicatedElement | Field::OutOfOrderSubstitution => {
            MutationOutcome::ExactError(InstallationError::NonCanonicalProviderPlanOrder)
        }
    }
}

#[test]
fn installation_selected_provider_plan_rejects_every_one_field_substitution() {
    let provider = WriteExitProvider(7);
    let plan = port_effect_plan(&provider);
    let artifact = build_object_artifact(&plan).expect("port-effect artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("port-effect image");
    let profile = ProfileDecisionId::new(23).expect("profile decision");
    let record = honest_selected_provider_plan_record(&provider, &image, profile);
    assert_eq!(
        record
            .selected_provider_plans()
            .iter()
            .map(|plan| plan.get())
            .collect::<Vec<_>>(),
        [7, 42]
    );
    validate_installation_record(&record, &image).expect("exact image binding");

    // Encode/decode preserves a substitution; the image join re-binds every
    // image-visible axis, and an accepted substitution is answered by the
    // honest record the published identity replays.
    let check = |record: &InstallationRecord| -> Result<InstallationRecord, InstallationError> {
        let bytes = encode_installation_record(record)?;
        let replayed = decode_installation_record(&bytes)?;
        assert_eq!(&replayed, record, "codec preserves the substituted record");
        validate_installation_record(&replayed, &image)?;
        Ok(honest_selected_provider_plan_record(
            &provider, &image, profile,
        ))
    };
    // Every unexecuted-selection substitution is itself a genuine admitted
    // selection producing the same record.
    let joined_replay = |record: &InstallationRecord, field: SelectedProviderPlanFieldForTest| {
        use SelectedProviderPlanFieldForTest as Field;
        match field {
            Field::UnexecutedElement | Field::ExtendedSelection | Field::DroppedUnexecuted => {
                let admitted = build_installation_record_with_selected_provider_plans_and_evidence(
                    &image,
                    profile,
                    record
                        .selected_provider_plans()
                        .iter()
                        .map(|plan| plan.get()),
                    [&provider],
                    None,
                    boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
                )
                .expect("substituted roster admits");
                assert_eq!(
                    record, &admitted,
                    "substitution is itself an admitted selection"
                );
            }
            _ => {}
        }
    };
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installation selected provider-plan roster",
        fields: SelectedProviderPlanFieldForTest::INVENTORY,
        honest: &|| honest_selected_provider_plan_record(&provider, &image, profile),
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_selected_provider_plan,
        check: &check,
        outcome: &selected_provider_plan_outcome,
        joined_replay: Some(&joined_replay),
    });

    // Admission binds the roster both ways: every reported execution must sit
    // inside the selected closure, and the reported closure must match the
    // image's retained executions exactly.
    assert_eq!(
        build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            [42],
            [&provider],
            None,
            boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        ),
        Err(InstallationError::ProviderExecutionOutsideSelectedClosure)
    );
    assert_eq!(
        build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            [7, 970],
            [&provider, &WriteExitProvider(970)],
            None,
            boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        ),
        Err(InstallationError::ProviderExecutionClosureMismatch)
    );
    assert_eq!(
        build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            [7, 0],
            [&provider],
            None,
            boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        ),
        Err(InstallationError::ZeroProviderPlan)
    );

    // Axes without an in-memory representation still reject at the wire: a
    // zero identity, a non-canonical or duplicated order, and a count the
    // remaining bytes cannot carry are malformed encodings, never a canonical
    // record.
    let canonical = encode_installation_record(&record).expect("canonical bytes");
    let plan_pair = [7_u64.to_le_bytes(), 42_u64.to_le_bytes()].concat();
    let plan_offset = canonical
        .windows(plan_pair.len())
        .position(|window| window == plan_pair.as_slice())
        .expect("adjacent provider-plan identities");
    let count_offset = plan_offset - 4;
    assert_eq!(
        u32::from_le_bytes(canonical[count_offset..plan_offset].try_into().unwrap()),
        2
    );
    let mut zero_identity = canonical.clone();
    zero_identity[plan_offset..plan_offset + 8].fill(0);
    assert_eq!(
        decode_installation_record(&zero_identity),
        Err(InstallationError::ZeroProviderPlan)
    );
    let mut wire_reordered = canonical.clone();
    wire_reordered[plan_offset..plan_offset + 8].copy_from_slice(&42_u64.to_le_bytes());
    wire_reordered[plan_offset + 8..plan_offset + 16].copy_from_slice(&7_u64.to_le_bytes());
    assert_eq!(
        decode_installation_record(&wire_reordered),
        Err(InstallationError::NonCanonicalProviderPlanOrder)
    );
    let mut wire_duplicated = canonical.clone();
    wire_duplicated[plan_offset + 8..plan_offset + 16].copy_from_slice(&7_u64.to_le_bytes());
    assert_eq!(
        decode_installation_record(&wire_duplicated),
        Err(InstallationError::NonCanonicalProviderPlanOrder)
    );
    let mut inflated_count = canonical.clone();
    inflated_count[count_offset..plan_offset].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        decode_installation_record(&inflated_count),
        Err(InstallationError::UnexpectedEnd)
    );
}

// Every representable field of an installed structural-return row is an
// authenticated custody axis: a one-field substitution either cannot encode
// canonically or still encodes, recomputes a distinct installation
// fingerprint, and independent replay against the unchanged image rejects it.
// The fixture retains both admitted lanes side by side: machine 1 is the
// claim-free affine identity family and machine 3 the claim-bearing linear
// family, each bound to its exact `mov rax, rdi; ret` interval by
// byte-regenerating object replay.
optimization_core::custody_field_inventory! {
    /// One substitutable axis of the installed structural-return roster. The
    /// result place is bound only by distinctness from the source place and
    /// the linear lane's carried claim identity only by its count, so those
    /// legs and a dropped roster row still encode and are rejected by
    /// independent replay. Every other field is a canonical projection bound
    /// by the record's own joins — the function roster pins `machine` and
    /// `byte_count`, the attribution roster pins `psi_edge` and the cleanup
    /// ordinals, the call plan pins every placement, and the
    /// source/result/lane consistency rules pin the rest — so those
    /// substitutions are rejected at encoding.
    enum InstalledStructuralReturnFieldForTest {
        ResultPlaceAffine,
        ResultPlaceLinear,
        ReturnedClaimLinear,
        RosterDropped,
        MachineOtherFunction,
        MachineMissing,
        PsiEdge,
        ScalarParameters,
        Parameters,
        ParametersExtended,
        ParameterPlacements,
        SourcePlace,
        SourcePosition,
        SourceIsSelf,
        SourceStructuralType,
        SourceMultiplicity,
        SourceAccess,
        SourceQualifications,
        SourceProjectedQualifications,
        ResultStructuralType,
        ResultMultiplicityLinear,
        ResultQualifications,
        ResultReferenceSources,
        Shape,
        SourcePlacement,
        ResultPlacement,
        ReturnedClaimsAffine,
        ReturnedClaimsDroppedLinear,
        TrivialAffineLocals,
        TrivialAffineDiscards,
        CodeOffset,
        ByteCount,
        RosterSwap,
        RosterDuplicate,
    }
}

/// An honestly produced structural-return installation record: the matrix's
/// staging is deterministic, so each leg re-derives the identical record.
fn honest_structural_return_record() -> InstallationRecord {
    let artifact =
        build_object_artifact(&structural_return_plan()).expect("structural-return artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("structural-return image");
    build_installation_record(&image, ProfileDecisionId::new(11).expect("profile"))
        .expect("structural-return installation")
}

/// Mutate exactly one structural-return axis; lane-owned legs index into the
/// affine (`0`) or linear (`1`) retained row. `donor` grants no new authority
/// and is retained for signature uniformity.
fn substitute_structural_return(
    record: &mut InstallationRecord,
    field: InstalledStructuralReturnFieldForTest,
    _donor: &InstallationRecord,
) {
    use InstalledStructuralReturnFieldForTest as Field;
    match field {
        Field::ResultPlaceAffine | Field::ResultPlaceLinear => {
            let index = match field {
                Field::ResultPlaceAffine => 0,
                _ => 1,
            };
            record.structural_returns_mut_for_test()[index]
                .returned
                .result
                .place = PlaceId::new(77).expect("substituted result place");
        }
        Field::ReturnedClaimLinear => {
            record.structural_returns_mut_for_test()[1]
                .returned
                .returned_claims[0] = ClaimId::new(32).expect("substituted claim identity");
        }
        Field::RosterDropped => {
            record.structural_returns_mut_for_test().pop();
        }
        Field::MachineOtherFunction => {
            record.structural_returns_mut_for_test()[0].machine = machine_id(2);
        }
        Field::MachineMissing => {
            record.structural_returns_mut_for_test()[0].machine = machine_id(99);
        }
        Field::PsiEdge => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .psi_edge = edge_id(9);
        }
        Field::ScalarParameters => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .scalar_parameters
                .push(target_operations::ScalarAbiValue {
                    value: semantic_vocabulary::ValueId::new(9).expect("scalar value"),
                    scalar_type: semantic_vocabulary::ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Signed,
                            32,
                        )
                        .expect("i32"),
                    ),
                    placement: ValuePlacement {
                        shape: ValueShape::integer(4, 4),
                        locations: Vec::new(),
                    },
                });
        }
        Field::Parameters => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .parameters[0]
                .place = PlaceId::new(55).expect("substituted parameter place");
        }
        Field::ParametersExtended => {
            let row = &mut record.structural_returns_mut_for_test()[0];
            let mut extra = row.returned.parameters[0].clone();
            extra.place = PlaceId::new(55).expect("extra parameter place");
            extra.position = 1;
            row.returned.parameters.push(extra);
        }
        Field::ParameterPlacements => {
            let row = &mut record.structural_returns_mut_for_test()[0];
            row.returned.parameter_placements[0] = row.returned.result_placement.clone();
        }
        Field::SourcePlace => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .source
                .place = PlaceId::new(55).expect("substituted source place");
        }
        Field::SourcePosition => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .source
                .position = 1;
        }
        Field::SourceIsSelf => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .source
                .is_self = true;
        }
        Field::SourceStructuralType => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .source
                .structural_type = StructuralTypeId::new(9).expect("substituted source type");
        }
        Field::SourceMultiplicity => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .source
                .multiplicity = StructuralMultiplicity::Linear;
        }
        Field::SourceAccess => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .source
                .access = StructuralAccess::SharedBorrow;
        }
        Field::SourceQualifications => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .source
                .qualifications
                .push(semantic_vocabulary::StructuralDomainId::new(1).expect("domain"));
        }
        Field::SourceProjectedQualifications => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .source
                .projected_qualifications
                .push(terminal_psi::StructuralPathQualification {
                    path: vec![terminal_psi::StructuralPathSegment::Field("field".into())],
                    domain: semantic_vocabulary::StructuralDomainId::new(1).expect("domain"),
                });
        }
        Field::ResultStructuralType => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .result
                .structural_type = StructuralTypeId::new(9).expect("substituted result type");
        }
        Field::ResultMultiplicityLinear => {
            record.structural_returns_mut_for_test()[1]
                .returned
                .result
                .multiplicity = StructuralMultiplicity::Affine;
        }
        Field::ResultQualifications => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .result
                .qualifications
                .push(semantic_vocabulary::StructuralDomainId::new(1).expect("domain"));
        }
        Field::ResultReferenceSources => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .result
                .reference_sources
                .push(terminal_psi::StructuralReferenceResultSource {
                    path: vec![terminal_psi::StructuralPathSegment::Field("field".into())],
                    source: StructuralArgument {
                        place: PlaceId::new(41).expect("reference source place"),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    },
                });
        }
        Field::Shape => {
            record.structural_returns_mut_for_test()[0].returned.shape = ValueShape::integer(16, 8);
        }
        Field::SourcePlacement => {
            let row = &mut record.structural_returns_mut_for_test()[0];
            row.returned.source_placement = row.returned.result_placement.clone();
        }
        Field::ResultPlacement => {
            let row = &mut record.structural_returns_mut_for_test()[0];
            row.returned.result_placement = row.returned.source_placement.clone();
        }
        Field::ReturnedClaimsAffine => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .returned_claims
                .push(ClaimId::new(32).expect("extra claim"));
        }
        Field::ReturnedClaimsDroppedLinear => {
            record.structural_returns_mut_for_test()[1]
                .returned
                .returned_claims
                .clear();
        }
        Field::TrivialAffineLocals => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .trivial_affine_locals
                .push((
                    operation_id(9),
                    terminal_psi::StructuralPlaceDeclaration {
                        id: PlaceId::new(61).expect("local place"),
                        kind: semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal: 0,
                            structural_type: StructuralTypeId::new(61).expect("local type"),
                            construction: None,
                        },
                    },
                    terminal_psi::StructuralTypeDeclaration {
                        id: StructuralTypeId::new(61).expect("local type"),
                        identity: "local".into(),
                        shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
                    },
                ));
        }
        Field::TrivialAffineDiscards => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .trivial_affine_discards
                .push(PlaceId::new(61).expect("discard place"));
        }
        Field::CodeOffset => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .code_offset += 1;
        }
        Field::ByteCount => {
            record.structural_returns_mut_for_test()[0]
                .returned
                .byte_count += 1;
        }
        Field::RosterSwap => {
            record.structural_returns_mut_for_test().swap(0, 1);
        }
        Field::RosterDuplicate => {
            let row = record.structural_returns()[0].clone();
            record.structural_returns_mut_for_test().insert(1, row);
        }
    }
}

/// The expected checker outcome per field: replay-bound legs surface as
/// `ImageBindingMismatch`, canonical-shape legs as the codec's rejection
/// attributed to the lane that owns them.
fn structural_return_outcome(
    field: InstalledStructuralReturnFieldForTest,
) -> MutationOutcome<InstallationError> {
    use InstalledStructuralReturnFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::ResultPlaceAffine
        | Field::ResultPlaceLinear
        | Field::ReturnedClaimLinear
        | Field::RosterDropped => InstallationError::ImageBindingMismatch,
        Field::MachineOtherFunction => InstallationError::InvalidStructuralReturn(machine_id(2)),
        Field::MachineMissing => InstallationError::StructuralReturnMachineMissing(machine_id(99)),
        Field::ResultMultiplicityLinear | Field::ReturnedClaimsDroppedLinear => {
            InstallationError::InvalidStructuralReturn(machine_id(3))
        }
        _ => InstallationError::InvalidStructuralReturn(machine_id(1)),
    })
}

#[test]
fn installation_structural_return_rejects_every_one_field_substitution() {
    let artifact =
        build_object_artifact(&structural_return_plan()).expect("structural-return artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("structural-return image");
    let record = honest_structural_return_record();
    validate_installation_record(&record, &image).expect("exact image binding");
    let [affine, linear] = record.structural_returns() else {
        panic!("fixture retains both structural-return lanes");
    };
    assert_eq!(affine.machine, machine_id(1));
    assert_eq!(linear.machine, machine_id(3));
    assert!(affine.returned.returned_claims.is_empty());
    assert_eq!(
        linear.returned.returned_claims,
        [ClaimId::new(31).expect("authentic claim")]
    );
    assert_eq!(affine.returned.result.place, PlaceId::new(42).unwrap());
    assert_eq!(linear.returned.result.place, PlaceId::new(52).unwrap());

    let check = installation_record_check(&image);
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "installed structural-return row",
        fields: InstalledStructuralReturnFieldForTest::INVENTORY,
        honest: &honest_structural_return_record,
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_structural_return,
        check: &check,
        outcome: &structural_return_outcome,
        joined_replay: None,
    });
}

#[test]
fn installation_decoder_rejects_alternate_and_malformed_encodings() {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("image");
    let record =
        build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).expect("record");
    let bytes = encode_installation_record(&record).expect("bytes");

    let mut future = bytes.clone();
    let future_marker = INSTALLATION_FORMAT_MARKER + 1;
    future[8..10].copy_from_slice(&future_marker.to_le_bytes());
    assert_eq!(
        decode_installation_record(&future),
        Err(InstallationError::UnsupportedFormatMarker(future_marker))
    );

    let mut wrong_pointer_width = bytes.clone();
    wrong_pointer_width[48..56].copy_from_slice(&4_u64.to_le_bytes());
    assert!(matches!(
        decode_installation_record(&wrong_pointer_width),
        Err(InstallationError::UnsupportedTarget(_))
    ));

    let mut zero_profile = bytes.clone();
    zero_profile[68..76].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        decode_installation_record(&zero_profile),
        Err(InstallationError::ZeroProfileDecision)
    );

    let mut changed_text_digest = bytes.clone();
    let compiler_text_validation = record.compiler_text_validation();
    let encoded_text_digest = compiler_text_validation.encoded_text_digest.as_bytes();
    let digest_offset = changed_text_digest
        .windows(encoded_text_digest.len())
        .position(|window| window == encoded_text_digest)
        .expect("encoded compiler-text digest");
    changed_text_digest[digest_offset] ^= 1;
    assert_eq!(
        decode_installation_record(&changed_text_digest),
        Err(InstallationError::InvalidCompilerTextDerivationDigest)
    );

    assert_eq!(
        decode_installation_record(&bytes[..bytes.len() - 1]),
        Err(InstallationError::UnexpectedEnd)
    );

    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode_installation_record(&trailing),
        Err(InstallationError::TrailingBytes(1))
    );
}

/// The record reader is the boundary between untrusted image bytes and the
/// installed-artifact contract: injected faults must be refused. No proper
/// prefix may decode a record that still binds to the image. A single-byte
/// fault must fail decode, decode to the honest record's equal only by
/// reproducing an already-canonical byte stream (impossible — the encode is
/// deterministic), or produce a record the image binding refuses. The sole
/// tolerated mutation is the profile-decision identity at bytes 68..76: it is
/// attribution provenance the image cannot contradict, and a faulted value
/// still describes an honestly bound record.
#[test]
fn installation_reader_rejects_every_truncation_and_byte_fault() {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("image");
    let record =
        build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).expect("record");
    let bytes = encode_installation_record(&record).expect("bytes");

    for len in 0..bytes.len() {
        if let Ok(prefix_record) = decode_installation_record(&bytes[..len]) {
            assert!(
                validate_installation_record(&prefix_record, &image).is_err(),
                "truncated prefix of {len} bytes must not bind to the image"
            );
        }
    }

    let mut ignored = Vec::new();
    let mut tolerated = Vec::new();
    for position in 0..bytes.len() {
        let mut faulted = bytes.clone();
        faulted[position] ^= 0xFF;
        if let Ok(faulted_record) = decode_installation_record(&faulted) {
            if faulted_record == record {
                ignored.push(position);
            } else if validate_installation_record(&faulted_record, &image).is_ok() {
                tolerated.push(position);
            }
        }
    }
    assert_eq!(
        ignored,
        Vec::<usize>::new(),
        "no encoded byte may be ignored"
    );
    // The profile-decision identity occupies bytes 68..76 in the format-99
    // header; validate_installation_record binds record↔image facts, so a
    // nonzero faulted identity remains a well-formed record of the same
    // installation. Everything else must be refused.
    assert_eq!(tolerated, (68..76).collect::<Vec<usize>>());
}
