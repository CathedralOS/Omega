use super::{
    TestComponentProgressAcceptance, WriteExitProvider, account_x86_unit_call, artifact_symbol,
    assert_header_substitution_rejected, edge_id, identity, integer_return, internal_call_plan,
    machine_id, operation_id, port_effect_plan, structural_return_plan,
    two_call_edge_owned_cleanup_plan, two_function_plan,
};
use calling_conventions::{ValuePlacement, ValueShape};
use image_emission::{
    INSTALLATION_FORMAT_MARKER, InstallationError, build_installation_record,
    build_installation_record_with_evidence,
    build_installation_record_with_selected_provider_plans_and_evidence, build_object_artifact,
    decode_installation_record, derive_installation_stack_demand, emit_executable_image,
    encode_installation_record, installation_fingerprint, validate_installation_record,
};
use installation_evidence::ProviderExecutionEvidence;
use machine_code::{MachineCodeFunction, MachineCodePlan};
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
        let image = emit_executable_image(&artifact, 3)
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
    let image = emit_executable_image(&artifact, 3).expect("Linux image");
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
    let changed_image = emit_executable_image(&changed_artifact, 3).expect("changed Linux image");
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
    let image = emit_executable_image(&artifact, 3).expect("Linux image");
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
    let macho_image = emit_executable_image(&macho_artifact, 3).expect("Mach-O image");
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

/// Every representable installation-header axis — program identity, target,
/// subsystem, profile decision, the committed component-progress projection,
/// the bound image fingerprint and section layout, and the compiler
/// text-validation receipt — is authenticated custody: a one-field
/// substitution either cannot encode canonically or still encodes, recomputes
/// a distinct installation fingerprint, and independent replay rejects it.
/// Axes bound to the emitted image reject through `validate_installation_record`.
/// The admission-owned axes — the caller-supplied profile decision and the
/// component-progress identities — are not image facts, so the image join
/// cannot see them; their custody is the published record identity that a
/// deployment journal replays against its pinned fingerprint.
#[test]
fn installation_header_rejects_every_one_field_substitution() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("Linux image");
    let record = build_installation_record_with_evidence(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        Some(&TestComponentProgressAcceptance {
            manifest: 0x1122,
            acceptance: 0x3344,
        }),
    )
    .expect("installation record");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic_evidence = record.compiler_text_validation();
    let authentic_progress = record
        .component_progress()
        .expect("committed component progress");
    assert_eq!(authentic_progress.manifest_identity(), 0x1122);
    assert_eq!(authentic_progress.acceptance_identity(), 0x3344);
    assert_eq!(record.subsystem(), None);

    // A relocation-bearing sibling image supplies well-typed foreign values
    // for the bound image fingerprint and the receipt digest axes.
    let mut other_plan = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut other_plan);
    let other_artifact = build_object_artifact(&other_plan).expect("other artifact");
    let other_image = emit_executable_image(&other_artifact, 3).expect("other image");
    let other =
        build_installation_record(&other_image, ProfileDecisionId::new(7).expect("profile"))
            .expect("other record");
    let other_fingerprint = other.image();
    let other_evidence = other.compiler_text_validation();
    assert_ne!(other_fingerprint, record.image());
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
    .component_progress();
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
    .component_progress();
    assert_ne!(manifest_only, record.component_progress());
    assert_ne!(acceptance_only, record.component_progress());

    // Axes joined to the emitted image: each substitution still encodes and
    // round-trips, recomputes a distinct installation fingerprint, and replay
    // against the unchanged image rejects it.
    let image_bound: Vec<(&str, Box<dyn Fn(&mut image_emission::InstallationRecord)>)> = vec![
        (
            "psi.program_fingerprint",
            Box::new(|record| {
                record.psi_mut_for_test().program_fingerprint =
                    SemanticFingerprint::from_bytes([0xa5; 32]);
            }),
        ),
        (
            "target.architecture",
            Box::new(|record| {
                record.target_mut_for_test().architecture = target::Architecture::Aarch64;
            }),
        ),
        (
            "target",
            Box::new(|record| {
                *record.target_mut_for_test() = NativeTarget::macos_arm64();
            }),
        ),
        (
            "image",
            Box::new(move |record| {
                *record.image_mut_for_test() = other_fingerprint;
            }),
        ),
        (
            "image_sections.layout.text_address",
            Box::new(|record| {
                record.image_sections_mut_for_test().layout.text_address += 0x1000;
            }),
        ),
        (
            "image_sections.layout.data_address",
            Box::new(|record| {
                record.image_sections_mut_for_test().layout.data_address += 0x1000;
            }),
        ),
        (
            "image_sections.layout.bss_address",
            Box::new(|record| {
                record.image_sections_mut_for_test().layout.bss_address += 0x1000;
            }),
        ),
        // The complete-custody extents and sealed inventory digests remain
        // representable above their floor; only the image join exposes drift.
        (
            "image_sections.final_text_byte_count",
            Box::new(|record| {
                record.image_sections_mut_for_test().final_text_byte_count += 8;
            }),
        ),
        (
            "image_sections.final_data_byte_count",
            Box::new(|record| {
                record.image_sections_mut_for_test().final_data_byte_count += 8;
            }),
        ),
        (
            "image_sections.executable_inventory_digest",
            Box::new(|record| {
                record
                    .image_sections_mut_for_test()
                    .executable_inventory_digest =
                    image::PlacedExecutableRegionInventoryDigest::from_digest([0x55; 32]);
            }),
        ),
        (
            "image_sections.data_inventory_digest",
            Box::new(|record| {
                record.image_sections_mut_for_test().data_inventory_digest =
                    image::PlacedDataRegionInventoryDigest::from_digest([0x55; 32]);
            }),
        ),
        (
            "compiler_text_validation",
            Box::new(move |record| {
                *record.compiler_text_validation_mut_for_test() = other_evidence;
            }),
        ),
        // `derivation_report_fingerprint` is report compatibility only: it is
        // outside the derivation-digest join, so the substitution is
        // representable without recomputing the receipt's own identity.
        (
            "compiler_text_validation.derivation_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .derivation_report_fingerprint += 1;
            }),
        ),
    ];
    for (field, mutate) in image_bound {
        assert_header_substitution_rejected(&record, &image, authentic_fingerprint, field, mutate);
    }

    // Every receipt input joined by the derivation digest remains
    // representable once the containing identity is honestly recomputed:
    // encoding accepts the consistent receipt and replay still rejects it
    // against the unchanged image.
    let recomputed_receipt: Vec<(&str, Box<dyn Fn(&mut image_emission::InstallationRecord)>)> = vec![
        (
            "compiler_text_validation.encoded_text_digest",
            Box::new(move |record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.encoded_text_digest = other_evidence.encoded_text_digest;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.final_compiler_text_digest",
            Box::new(move |record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.final_compiler_text_digest = other_evidence.final_compiler_text_digest;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.relocation_envelope_digest",
            Box::new(move |record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.relocation_envelope_digest = other_evidence.relocation_envelope_digest;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.encoded_text_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.encoded_text_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.final_compiler_text_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.final_compiler_text_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.relocation_envelope_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.relocation_envelope_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.checked_instruction_validation_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.checked_instruction_validation_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.checked_instruction_footprint_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.checked_instruction_footprint_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.text_relocation_count",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.text_relocation_count += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.checked_instruction_validation_count",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.checked_instruction_validation_count += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
    ];
    for (field, mutate) in recomputed_receipt {
        assert_header_substitution_rejected(&record, &image, authentic_fingerprint, field, mutate);
    }

    // Admission-owned axes are not image facts: the image join cannot see
    // them, so the substitution only breaks the published record identity —
    // the recomputed-fingerprint mismatch a deployment journal applies.
    let admission_bound: Vec<(&str, Box<dyn Fn(&mut image_emission::InstallationRecord)>)> = vec![
        (
            "profile_decision",
            Box::new(|record| {
                *record.profile_decision_mut_for_test() =
                    ProfileDecisionId::new(12).expect("profile decision");
            }),
        ),
        (
            "component_progress.manifest",
            Box::new(move |record| {
                *record.component_progress_mut_for_test() = manifest_only;
            }),
        ),
        (
            "component_progress.acceptance",
            Box::new(move |record| {
                *record.component_progress_mut_for_test() = acceptance_only;
            }),
        ),
        (
            "component_progress",
            Box::new(|record| {
                *record.component_progress_mut_for_test() = None;
            }),
        ),
    ];
    for (field, mutate) in admission_bound {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Ok(()),
            "{field}: admission-owned axes sit outside the image join"
        );
    }

    // The remaining header axes are not independently representable:
    // canonical encoding rejects them before any identity or replay check.
    let encode_rejected: Vec<(
        &str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    )> = vec![
        // ELF and Mach-O records carry no subsystem fact.
        (
            "subsystem",
            Box::new(|record| {
                *record.subsystem_mut_for_test() = Some(3);
            }),
            InstallationError::UnexpectedSubsystem,
        ),
        // `object_format` has no representable substitution on an x86-64
        // record: Mach-O emits only for AArch64 and COFF requires a
        // subsystem.
        (
            "target.object_format",
            Box::new(|record| {
                record.target_mut_for_test().object_format = target::ObjectFormat::MachO;
            }),
            InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::MachO,
                pointer_size: 8,
                pointer_alignment: 8,
            }),
        ),
        (
            "target.object_format",
            Box::new(|record| {
                record.target_mut_for_test().object_format = target::ObjectFormat::Coff;
            }),
            InstallationError::MissingCoffSubsystem,
        ),
        (
            "target.pointer_size",
            Box::new(|record| {
                record.target_mut_for_test().pointer_size = 4;
            }),
            InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::Elf,
                pointer_size: 4,
                pointer_alignment: 8,
            }),
        ),
        (
            "target.pointer_alignment",
            Box::new(|record| {
                record.target_mut_for_test().pointer_alignment = 16;
            }),
            InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::Elf,
                pointer_size: 8,
                pointer_alignment: 16,
            }),
        ),
        // The section projection re-derives its facts from the retained
        // function roster and the initialized-data prefix.
        (
            "image_sections.layout.text_address",
            Box::new(|record| {
                record.image_sections_mut_for_test().layout.text_address = 0;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "image_sections.text_byte_count",
            Box::new(|record| {
                record.image_sections_mut_for_test().text_byte_count += 8;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "image_sections.data_byte_count",
            Box::new(|record| {
                record.image_sections_mut_for_test().data_byte_count += 8;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "image_sections.final_data_fingerprint",
            Box::new(|record| {
                record.image_sections_mut_for_test().final_data_fingerprint =
                    image_emission::InitializedDataFingerprint::for_test([0x33; 32]);
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        // The complete final extents can never contract below the
        // compiler-authored spans they contain, and neither sealed inventory
        // digest may be absent.
        (
            "image_sections.final_text_byte_count::below_compiler_text",
            Box::new(|record| {
                record.image_sections_mut_for_test().final_text_byte_count -= 1;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "image_sections.executable_inventory_digest::absent",
            Box::new(|record| {
                record
                    .image_sections_mut_for_test()
                    .executable_inventory_digest =
                    image::PlacedExecutableRegionInventoryDigest::from_digest([0; 32]);
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "image_sections.data_inventory_digest::absent",
            Box::new(|record| {
                record.image_sections_mut_for_test().data_inventory_digest =
                    image::PlacedDataRegionInventoryDigest::from_digest([0; 32]);
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        // Every receipt input bound by the derivation digest — and the digest
        // itself — is non-canonical without a consistent recomputation.
        (
            "compiler_text_validation.encoded_text_digest",
            Box::new(move |record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .encoded_text_digest = other_evidence.encoded_text_digest;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.final_compiler_text_digest",
            Box::new(move |record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .final_compiler_text_digest = other_evidence.final_compiler_text_digest;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.relocation_envelope_digest",
            Box::new(move |record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .relocation_envelope_digest = other_evidence.relocation_envelope_digest;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.derivation_digest",
            Box::new(move |record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .derivation_digest = other_evidence.derivation_digest;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.encoded_text_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .encoded_text_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.final_compiler_text_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .final_compiler_text_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.relocation_envelope_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .relocation_envelope_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.checked_instruction_validation_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .checked_instruction_validation_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.checked_instruction_footprint_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .checked_instruction_footprint_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.text_relocation_count",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .text_relocation_count += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.checked_instruction_validation_count",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .checked_instruction_validation_count += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
    ];
    for (field, mutate, expected) in encode_rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: substituted record is rejected at canonical encoding"
        );
    }

    // The subsystem axis is representable only where the writer records it:
    // on a PE/COFF image the substituted subsystem still encodes and replay
    // against the unchanged image rejects it.
    let mut coff_plan = two_function_plan();
    coff_plan.target = NativeTarget::windows_x64();
    let coff_artifact = build_object_artifact(&coff_plan).expect("COFF artifact");
    let coff_image = emit_executable_image(&coff_artifact, 3).expect("PE image");
    let coff_record =
        build_installation_record(&coff_image, ProfileDecisionId::new(19).expect("profile"))
            .expect("COFF record");
    assert_eq!(coff_record.subsystem(), Some(3));
    validate_installation_record(&coff_record, &coff_image).expect("COFF binding");
    let coff_fingerprint = installation_fingerprint(&coff_record).expect("COFF fingerprint");
    assert_header_substitution_rejected(
        &coff_record,
        &coff_image,
        coff_fingerprint,
        "subsystem",
        |record| {
            *record.subsystem_mut_for_test() = Some(4);
        },
    );
    for (field, mutate, expected) in [
        (
            "subsystem",
            Box::new(|record: &mut image_emission::InstallationRecord| {
                *record.subsystem_mut_for_test() = None;
            }) as Box<dyn Fn(&mut image_emission::InstallationRecord)>,
            InstallationError::MissingCoffSubsystem,
        ),
        // A retained subsystem is non-canonical on every non-COFF target.
        (
            "target",
            Box::new(|record: &mut image_emission::InstallationRecord| {
                *record.target_mut_for_test() = NativeTarget::linux_x64();
            }) as Box<dyn Fn(&mut image_emission::InstallationRecord)>,
            InstallationError::UnexpectedSubsystem,
        ),
        (
            "target.object_format",
            Box::new(|record: &mut image_emission::InstallationRecord| {
                record.target_mut_for_test().object_format = target::ObjectFormat::MachO;
            }) as Box<dyn Fn(&mut image_emission::InstallationRecord)>,
            InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::MachO,
                pointer_size: 8,
                pointer_alignment: 8,
            }),
        ),
    ] {
        let mut changed = coff_record.clone();
        mutate(&mut changed);
        assert_ne!(
            changed, coff_record,
            "{field}: substitution changes the record"
        );
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: substituted COFF record is rejected at canonical encoding"
        );
    }

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

/// Every representable scalar field of an installed function row is an
/// authenticated custody axis: a one-field substitution still encodes,
/// recomputes a distinct installation fingerprint, and independent replay
/// against the unchanged image rejects it. Roster-level rows (call stacks,
/// homes, continuations) are covered by their producing fixtures elsewhere.
#[test]
fn installation_function_row_rejects_every_one_field_substitution() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("Linux image");
    let record = build_installation_record(&image, ProfileDecisionId::new(11).expect("profile"))
        .expect("installation record");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let [_, authentic] = record.functions() else {
        panic!("two-function fixture retains two rows");
    };
    assert_eq!(authentic.attachment, None);
    assert!(!authentic.unit_body);
    assert_eq!(authentic.unit_stack, None);
    assert_eq!(authentic.scalar_stack, None);

    let mutations: [(&str, fn(&mut image_emission::InstalledFunction)); 4] = [
        ("machine", |row| {
            row.machine = MachineId::new(row.machine.get() + 100).expect("drifted machine");
        }),
        ("attachment", |row| {
            row.attachment = Some(StructuralTypeId::new(7).expect("drifted attachment"));
        }),
        ("unit_stack", |row| {
            row.unit_stack = Some(image_emission::ObjectUnitStack {
                frame_bytes: 16,
                local_peak_bytes: 0,
                stack_alignment: 16,
            });
        }),
        ("scalar_stack", |row| {
            row.scalar_stack = Some(image_emission::ObjectScalarStack {
                local_peak_bytes: 0,
                stack_alignment: 16,
            });
        }),
    ];
    for (field, mutate) in mutations {
        let mut changed = record.clone();
        mutate(&mut changed.functions_mut_for_test()[1]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted row encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted row decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted row"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted row"
        );
    }

    // Text intervals are not independently representable: canonical rows are
    // contiguous and exhaust the text section, so the encoder rejects a shifted
    // offset, a shortened interval, or a dropped row before any identity or
    // replay could accept it.
    let mut shifted_offset = record.clone();
    shifted_offset.functions_mut_for_test()[1].text_offset += 1;
    assert_eq!(
        encode_installation_record(&shifted_offset),
        Err(InstallationError::NonCanonicalInstalledFunctions)
    );
    let mut shortened = record.clone();
    shortened.functions_mut_for_test()[1].byte_count -= 1;
    assert_eq!(
        encode_installation_record(&shortened),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    let mut dropped_row = record.clone();
    dropped_row.functions_mut_for_test().pop();
    assert_eq!(
        encode_installation_record(&dropped_row),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    // `unit_body` is a projection of the retained affine cleanup; flipping it
    // alone is likewise rejected at encoding.
    let mut flipped_body = record.clone();
    flipped_body.functions_mut_for_test()[1].unit_body = true;
    assert_eq!(
        encode_installation_record(&flipped_body),
        Err(InstallationError::InvalidUnitAffineCleanup(machine_id(2)))
    );
}

/// Every representable field of an installed internal Unit-call row is an
/// authenticated custody axis: a one-field substitution either cannot encode
/// canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects it.
#[test]
fn installation_internal_unit_call_row_rejects_every_one_field_substitution() {
    let plan = two_call_edge_owned_cleanup_plan();
    let artifact = build_object_artifact(&plan).expect("cleanup artifact");
    let image = emit_executable_image(&artifact, 3).expect("cleanup image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("cleanup installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
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

    let i32_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32");
    let empty_placement = ValuePlacement {
        shape: ValueShape::integer(0, 1),
        locations: Vec::new(),
    };
    let scalar_placement = ValuePlacement {
        shape: ValueShape::integer(4, 4),
        locations: Vec::new(),
    };
    let provider_source = machine_code::InternalUnitCallSource::InstalledProvider {
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
    };
    let affine_structural_result = machine_code::InternalStructuralCallResult {
        operation_result: terminal_psi::StructuralOperationResult {
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
        callee_result_placement: empty_placement.clone(),
    };
    let scalar_argument = machine_code::InternalUnitScalarCallArgumentRecord {
        parameter_index: 0,
        source: machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation: operation_id(1),
            source_value: semantic_vocabulary::ValueId::new(31).unwrap(),
            scalar_type: i32_type,
            value: semantic_vocabulary::IntegerValue::Signed(7),
        },
        destination: scalar_placement.clone(),
        code_offset: 0,
        byte_count: 4,
    };
    let structural_argument = machine_code::InternalUnitCallArgumentRecord {
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
    };

    use std::rc::Rc;
    let provider_source = Rc::new(provider_source);
    let affine_structural_result = Rc::new(affine_structural_result);
    let scalar_argument = Rc::new(scalar_argument);
    let structural_argument = Rc::new(structural_argument);

    // A cleanup-owned call's `byte_count` is the one field no record-shape
    // join pins: the cleanup action's extent is carried by the cleanup
    // record itself, so the substituted row still encodes. It recomputes a
    // distinct installation fingerprint, and independent replay against the
    // unchanged image rejects it.
    let mut changed = record.clone();
    changed.internal_unit_calls_mut_for_test()[2]
        .custody
        .byte_count += 1;
    let bytes =
        encode_installation_record(&changed).expect("substituted cleanup byte_count encodes");
    let replayed =
        decode_installation_record(&bytes).expect("substituted cleanup byte_count decodes");
    assert_eq!(replayed, changed);
    assert_ne!(
        installation_fingerprint(&replayed).expect("substituted fingerprint"),
        authentic_fingerprint,
        "byte_count: recomputed identity differs from the authentic record"
    );
    assert_eq!(
        validate_installation_record(&replayed, &image),
        Err(InstallationError::ImageBindingMismatch),
        "byte_count: independent replay rejects the substituted row"
    );

    // Every other field on every row is a canonical projection bound by the
    // caller's attribution and cleanup joins or the callee's own call shape,
    // so a one-field substitution is rejected at encoding before any identity
    // or replay could accept it.
    let provider_source_owned = provider_source.clone();
    let provider_source_cleanup = provider_source.clone();
    let affine_structural_result_owned = affine_structural_result.clone();
    let affine_structural_result_cleanup = affine_structural_result.clone();
    let scalar_argument_owned = scalar_argument.clone();
    let scalar_argument_cleanup = scalar_argument.clone();
    let scalar_argument_second = scalar_argument.clone();
    let structural_argument_owned = structural_argument.clone();
    let structural_argument_cleanup = structural_argument.clone();

    type CallMutation = (
        &'static str,
        usize,
        Box<dyn Fn(&mut image_emission::InstalledInternalUnitCall)>,
        InstallationError,
    );
    let cleanup_join = InstallationError::InvalidUnitAffineCleanup(machine_id(3));
    let mutations: Vec<CallMutation> = vec![
        (
            "machine",
            0,
            Box::new(|row| {
                row.machine = machine_id(2);
            }),
            cleanup_join.clone(),
        ),
        (
            "text_offset",
            0,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "owner",
            0,
            Box::new(|row| {
                row.custody.owner = CallSiteOwner::Operation(operation_id(7));
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "owner::cleanup_action",
            2,
            Box::new(|row| {
                row.custody.owner = CallSiteOwner::CleanupAction {
                    edge: edge_id(4),
                    action_ordinal: 1,
                };
            }),
            cleanup_join.clone(),
        ),
        (
            "source",
            0,
            Box::new(move |row| {
                row.custody.source = (*provider_source_owned).clone();
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "target",
            0,
            Box::new(|row| {
                row.custody.target = machine_id(4);
            }),
            cleanup_join.clone(),
        ),
        (
            "result",
            0,
            Box::new(|row| {
                row.custody.result = Some(semantic_vocabulary::ScalarType::Boolean);
            }),
            cleanup_join.clone(),
        ),
        (
            "structural_result",
            0,
            Box::new(move |row| {
                row.custody.structural_result = Some((*affine_structural_result_owned).clone());
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_arguments",
            0,
            Box::new(move |row| {
                row.custody
                    .scalar_arguments
                    .push((*scalar_argument_owned).clone());
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "arguments",
            0,
            Box::new(move |row| {
                row.custody
                    .arguments
                    .push((*structural_argument_owned).clone());
            }),
            cleanup_join.clone(),
        ),
        (
            "claim_transfers",
            0,
            Box::new(|row| {
                row.custody
                    .claim_transfers
                    .push(terminal_psi::ClaimTransfer {
                        claim: ClaimId::new(31).unwrap(),
                        argument_index: 0,
                    });
            }),
            cleanup_join.clone(),
        ),
        (
            "operation_ordinal",
            0,
            Box::new(|row| {
                row.custody.operation_ordinal += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "code_offset",
            0,
            Box::new(|row| {
                row.custody.code_offset += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "code_offset::second_call",
            1,
            Box::new(|row| {
                row.custody.code_offset += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "byte_count",
            0,
            Box::new(|row| {
                row.custody.byte_count += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "byte_count::second_call",
            1,
            Box::new(|row| {
                row.custody.byte_count += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "machine::cleanup_call",
            2,
            Box::new(|row| {
                row.machine = machine_id(4);
            }),
            cleanup_join.clone(),
        ),
        (
            "text_offset::cleanup_call",
            2,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "code_offset::cleanup_call",
            2,
            Box::new(|row| {
                row.custody.code_offset += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "operation_ordinal::cleanup_call",
            2,
            Box::new(|row| {
                row.custody.operation_ordinal += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "source::cleanup_call",
            2,
            Box::new(move |row| {
                row.custody.source = (*provider_source_cleanup).clone();
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "result::cleanup_call",
            2,
            Box::new(|row| {
                row.custody.result = Some(semantic_vocabulary::ScalarType::Boolean);
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "scalar_arguments::cleanup_call",
            2,
            Box::new(move |row| {
                row.custody
                    .scalar_arguments
                    .push((*scalar_argument_cleanup).clone());
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "arguments::cleanup_call",
            2,
            Box::new(move |row| {
                row.custody
                    .arguments
                    .push((*structural_argument_cleanup).clone());
            }),
            cleanup_join.clone(),
        ),
        (
            "structural_result::cleanup_call",
            2,
            Box::new(move |row| {
                row.custody.structural_result = Some((*affine_structural_result_cleanup).clone());
            }),
            cleanup_join.clone(),
        ),
        (
            "target::second_call",
            1,
            Box::new(|row| {
                row.custody.target = machine_id(2);
            }),
            cleanup_join.clone(),
        ),
        (
            "operation_ordinal::second_call",
            1,
            Box::new(|row| {
                row.custody.operation_ordinal += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "scalar_arguments::second_call",
            1,
            Box::new(move |row| {
                row.custody
                    .scalar_arguments
                    .push((*scalar_argument_second).clone());
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "target::cleanup_call",
            2,
            Box::new(|row| {
                row.custody.target = machine_id(4);
            }),
            cleanup_join.clone(),
        ),
        (
            "claim_transfers::cleanup_call",
            2,
            Box::new(|row| {
                row.custody
                    .claim_transfers
                    .push(terminal_psi::ClaimTransfer {
                        claim: ClaimId::new(31).unwrap(),
                        argument_index: 0,
                    });
            }),
            cleanup_join.clone(),
        ),
        (
            "dropped_row",
            usize::MAX,
            Box::new(|_| {}),
            cleanup_join.clone(),
        ),
    ];
    for (field, row_index, mutate, expected) in mutations {
        let mut changed = record.clone();
        if row_index == usize::MAX {
            changed.internal_unit_calls_mut_for_test().remove(0);
        } else {
            mutate(&mut changed.internal_unit_calls_mut_for_test()[row_index]);
        }
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: substituted row is rejected at canonical encoding"
        );
    }

    // `semantic_result` is a canonical projection of `result`: a value whose
    // scalar type does not equal the retained ABI result is rejected at
    // encoding rather than reaching replay.
    let mut changed = record.clone();
    changed.internal_unit_calls_mut_for_test()[0]
        .custody
        .semantic_result = Some(abstract_operations::AbstractResult {
        value: semantic_vocabulary::ValueId::new(31).unwrap(),
        scalar_type: semantic_vocabulary::ScalarType::Boolean,
    });
    assert_eq!(
        encode_installation_record(&changed),
        Err(InstallationError::InvalidInternalUnitCall(machine_id(1)))
    );
}

#[test]
fn installation_record_fingerprints_component_progress_acceptance() {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("image");
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
    let image = emit_executable_image(&artifact, 3).expect("image");
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

/// The selected provider-plan roster is admission-owned custody. The executed
/// subset is pinned by the boundary settlements' admitted-provider execution
/// records: substituting or dropping the executed plan, clearing or reordering
/// the roster, or duplicating an entry is rejected at canonical encoding. The
/// unexecuted remainder is representable — exact selection authority stays
/// outside the decodable record — so each substitution still encodes,
/// recomputes a distinct installation fingerprint, and is rejected by the
/// published record identity a deployment journal replays rather than by the
/// image join. Malformed identities and non-canonical wire order reject at
/// decode.
#[test]
fn installation_selected_provider_plan_rejects_every_one_field_substitution() {
    let provider = WriteExitProvider(7);
    let plan = port_effect_plan(&provider);
    let artifact = build_object_artifact(&plan).expect("port-effect artifact");
    let image = emit_executable_image(&artifact, 3).expect("port-effect image");
    let profile = ProfileDecisionId::new(23).expect("profile decision");
    // The settlement's admitted-provider execution requires plan 7; plan 42 is
    // selected but never executes in this image.
    let record = build_installation_record_with_selected_provider_plans_and_evidence(
        &image,
        profile,
        [7, 42],
        [&provider],
        None,
        boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
    )
    .expect("selected closure with an unexecuted plan");
    assert_eq!(
        record
            .selected_provider_plans()
            .iter()
            .map(|plan| plan.get())
            .collect::<Vec<_>>(),
        [7, 42]
    );
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");

    // Every unexecuted-selection substitution is independently representable:
    // the mutated roster is itself a genuine admitted selection producing the
    // same record, the substitution still encodes and round-trips, and the
    // recomputed installation fingerprint differs from the authentic record —
    // the published identity a deployment journal replays. The image join
    // cannot see the change, so image replay still accepts.
    let representable: [(&str, &[u64]); 3] = [
        ("unexecuted element", &[7, 43]),
        ("extended selection", &[7, 42, 99]),
        ("dropped unexecuted", &[7]),
    ];
    for (field, plans) in representable {
        let mut changed = record.clone();
        *changed.selected_provider_plans_mut_for_test() = plans
            .iter()
            .map(|plan| {
                image_emission::SelectedProviderPlanReportIdentity::new(*plan)
                    .expect("nonzero plan identity")
            })
            .collect();
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let admitted = build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            plans.iter().copied(),
            [&provider],
            None,
            boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        )
        .unwrap_or_else(|error| panic!("{field}: substituted roster admits: {error:?}"));
        assert_eq!(
            changed, admitted,
            "{field}: substitution is itself an admitted selection"
        );
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Ok(()),
            "{field}: the selected closure sits outside the image join"
        );
    }

    // The executed subset is bound by the record-shape closure join: a
    // substitution that loses the required plan, an emptied roster, a
    // non-canonical order, or a duplicated entry all fail at encoding before
    // any identity or replay check.
    let encode_rejected: [(
        &str,
        fn(&mut image_emission::InstallationRecord),
        InstallationError,
    ); 6] = [
        (
            "executed element",
            |record| {
                record.selected_provider_plans_mut_for_test()[0] =
                    image_emission::SelectedProviderPlanReportIdentity::new(8)
                        .expect("nonzero plan identity");
            },
            InstallationError::ProviderSettlementClosureMismatch,
        ),
        (
            "dropped executed",
            |record| {
                record.selected_provider_plans_mut_for_test().remove(0);
            },
            InstallationError::ProviderSettlementClosureMismatch,
        ),
        (
            "cleared roster",
            |record| {
                record.selected_provider_plans_mut_for_test().clear();
            },
            InstallationError::ProviderSettlementClosureMismatch,
        ),
        (
            "reordered roster",
            |record| {
                record.selected_provider_plans_mut_for_test().swap(0, 1);
            },
            InstallationError::NonCanonicalProviderPlanOrder,
        ),
        (
            "duplicated element",
            |record| {
                let plans = record.selected_provider_plans_mut_for_test();
                plans.insert(1, plans[0]);
            },
            InstallationError::NonCanonicalProviderPlanOrder,
        ),
        (
            "out-of-order substitution",
            |record| {
                record.selected_provider_plans_mut_for_test()[1] =
                    image_emission::SelectedProviderPlanReportIdentity::new(3)
                        .expect("nonzero plan identity");
            },
            InstallationError::NonCanonicalProviderPlanOrder,
        ),
    ];
    for (field, mutate, expected) in encode_rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: non-canonical substitution rejected at encoding"
        );
    }

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

/// Every representable field of an installed structural-return row is an
/// authenticated custody axis: a one-field substitution either cannot encode
/// canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects
/// it. The fixture retains both admitted lanes side by side: machine 1 is the
/// claim-free affine identity family and machine 3 the claim-bearing linear
/// family, each bound to its exact `mov rax, rdi; ret` interval by
/// byte-regenerating object replay.
#[test]
fn installation_structural_return_rejects_every_one_field_substitution() {
    let plan = structural_return_plan();
    let artifact = build_object_artifact(&plan).expect("structural-return artifact");
    let image = emit_executable_image(&artifact, 3).expect("structural-return image");
    let record = build_installation_record(&image, ProfileDecisionId::new(11).expect("profile"))
        .expect("structural-return installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
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

    // The result place is bound only by distinctness from the source place,
    // and the linear lane's carried claim identity is bound only by its
    // count, so each one-field substitution still encodes canonically,
    // recomputes a distinct installation fingerprint, and independent replay
    // against the unchanged image rejects it.
    type RepresentableMutation = (
        &'static str,
        usize,
        fn(&mut image_emission::InstalledStructuralReturn),
    );
    let representable: [RepresentableMutation; 3] = [
        ("result.place", 0, |row| {
            row.returned.result.place = PlaceId::new(77).expect("substituted result place");
        }),
        ("result.place::linear", 1, |row| {
            row.returned.result.place = PlaceId::new(77).expect("substituted result place");
        }),
        ("returned_claims", 1, |row| {
            row.returned.returned_claims[0] = ClaimId::new(32).expect("substituted claim identity");
        }),
    ];
    for (field, index, mutate) in representable {
        let mut changed = record.clone();
        mutate(&mut changed.structural_returns_mut_for_test()[index]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted row encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted row decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted row"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted row"
        );
    }

    // Every other field is a canonical projection bound by the record's own
    // joins — the function roster pins `machine` and `byte_count`, the
    // attribution roster pins `psi_edge` and the cleanup ordinals, the call
    // plan pins every placement, and the source/result/lane consistency rules
    // pin the rest — so a one-field substitution is rejected at encoding
    // before any identity or replay could accept it.
    let invalid_affine = InstallationError::InvalidStructuralReturn(machine_id(1));
    let invalid_linear = InstallationError::InvalidStructuralReturn(machine_id(3));
    let drifted_placement = ValuePlacement {
        shape: ValueShape::integer(4, 4),
        locations: Vec::new(),
    };
    let projected_qualification = terminal_psi::StructuralPathQualification {
        path: vec![terminal_psi::StructuralPathSegment::Field("field".into())],
        domain: semantic_vocabulary::StructuralDomainId::new(1).expect("domain"),
    };
    let trivial_local = (
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
    );
    type ReturnMutation = (
        &'static str,
        usize,
        Box<dyn Fn(&mut image_emission::InstalledStructuralReturn)>,
        InstallationError,
    );
    let mutations: Vec<ReturnMutation> = vec![
        (
            "machine::other_function",
            0,
            Box::new(|row| row.machine = machine_id(2)),
            InstallationError::InvalidStructuralReturn(machine_id(2)),
        ),
        (
            "machine::missing",
            0,
            Box::new(|row| row.machine = machine_id(99)),
            InstallationError::StructuralReturnMachineMissing(machine_id(99)),
        ),
        (
            "psi_edge",
            0,
            Box::new(|row| row.returned.psi_edge = edge_id(9)),
            invalid_affine.clone(),
        ),
        (
            "scalar_parameters",
            0,
            Box::new(move |row| {
                row.returned
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
                        placement: drifted_placement.clone(),
                    });
            }),
            invalid_affine.clone(),
        ),
        (
            "parameters",
            0,
            Box::new(|row| {
                row.returned.parameters[0].place =
                    PlaceId::new(55).expect("substituted parameter place");
            }),
            invalid_affine.clone(),
        ),
        (
            "parameters::extended",
            0,
            Box::new(|row| {
                let mut extra = row.returned.parameters[0].clone();
                extra.place = PlaceId::new(55).expect("extra parameter place");
                extra.position = 1;
                row.returned.parameters.push(extra);
            }),
            invalid_affine.clone(),
        ),
        (
            "parameter_placements",
            0,
            Box::new(|row| {
                row.returned.parameter_placements[0] = row.returned.result_placement.clone();
            }),
            invalid_affine.clone(),
        ),
        (
            "source.place",
            0,
            Box::new(|row| {
                row.returned.source.place = PlaceId::new(55).expect("substituted source place");
            }),
            invalid_affine.clone(),
        ),
        (
            "source.position",
            0,
            Box::new(|row| row.returned.source.position = 1),
            invalid_affine.clone(),
        ),
        (
            "source.is_self",
            0,
            Box::new(|row| row.returned.source.is_self = true),
            invalid_affine.clone(),
        ),
        (
            "source.structural_type",
            0,
            Box::new(|row| {
                row.returned.source.structural_type =
                    StructuralTypeId::new(9).expect("substituted source type");
            }),
            invalid_affine.clone(),
        ),
        (
            "source.multiplicity",
            0,
            Box::new(|row| {
                row.returned.source.multiplicity = StructuralMultiplicity::Linear;
            }),
            invalid_affine.clone(),
        ),
        (
            "source.access",
            0,
            Box::new(|row| row.returned.source.access = StructuralAccess::SharedBorrow),
            invalid_affine.clone(),
        ),
        (
            "source.qualifications",
            0,
            Box::new(|row| {
                row.returned
                    .source
                    .qualifications
                    .push(semantic_vocabulary::StructuralDomainId::new(1).expect("domain"));
            }),
            invalid_affine.clone(),
        ),
        (
            "source.projected_qualifications",
            0,
            Box::new(move |row| {
                row.returned
                    .source
                    .projected_qualifications
                    .push(projected_qualification.clone());
            }),
            invalid_affine.clone(),
        ),
        (
            "result.structural_type",
            0,
            Box::new(|row| {
                row.returned.result.structural_type =
                    StructuralTypeId::new(9).expect("substituted result type");
            }),
            invalid_affine.clone(),
        ),
        (
            "result.multiplicity",
            1,
            Box::new(|row| {
                row.returned.result.multiplicity = StructuralMultiplicity::Affine;
            }),
            invalid_linear.clone(),
        ),
        (
            "result.qualifications",
            0,
            Box::new(|row| {
                row.returned
                    .result
                    .qualifications
                    .push(semantic_vocabulary::StructuralDomainId::new(1).expect("domain"));
            }),
            invalid_affine.clone(),
        ),
        (
            "result.reference_sources",
            0,
            Box::new(|row| {
                row.returned.result.reference_sources.push(
                    terminal_psi::StructuralReferenceResultSource {
                        path: vec![terminal_psi::StructuralPathSegment::Field("field".into())],
                        source: StructuralArgument {
                            place: PlaceId::new(41).expect("reference source place"),
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        },
                    },
                );
            }),
            invalid_affine.clone(),
        ),
        (
            "shape",
            0,
            Box::new(|row| row.returned.shape = ValueShape::integer(16, 8)),
            invalid_affine.clone(),
        ),
        (
            "source_placement",
            0,
            Box::new(|row| {
                row.returned.source_placement = row.returned.result_placement.clone();
            }),
            invalid_affine.clone(),
        ),
        (
            "result_placement",
            0,
            Box::new(|row| {
                row.returned.result_placement = row.returned.source_placement.clone();
            }),
            invalid_affine.clone(),
        ),
        (
            "returned_claims::affine",
            0,
            Box::new(|row| {
                row.returned
                    .returned_claims
                    .push(ClaimId::new(32).expect("extra claim"));
            }),
            invalid_affine.clone(),
        ),
        (
            "returned_claims::dropped",
            1,
            Box::new(|row| row.returned.returned_claims.clear()),
            invalid_linear.clone(),
        ),
        (
            "trivial_affine_locals",
            0,
            Box::new(move |row| {
                row.returned
                    .trivial_affine_locals
                    .push(trivial_local.clone());
            }),
            invalid_affine.clone(),
        ),
        (
            "trivial_affine_discards",
            0,
            Box::new(|row| {
                row.returned
                    .trivial_affine_discards
                    .push(PlaceId::new(61).expect("discard place"));
            }),
            invalid_affine.clone(),
        ),
        (
            "code_offset",
            0,
            Box::new(|row| row.returned.code_offset += 1),
            invalid_affine.clone(),
        ),
        (
            "byte_count",
            0,
            Box::new(|row| row.returned.byte_count += 1),
            invalid_affine.clone(),
        ),
    ];
    for (field, index, mutate, expected) in mutations {
        let mut changed = record.clone();
        mutate(&mut changed.structural_returns_mut_for_test()[index]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: non-canonical substitution rejected at encoding"
        );
    }

    // The roster itself is canonical too: a machine-descending swap or a
    // duplicated row is rejected at encoding, while a dropped row still
    // encodes and independent replay rejects the thinned roster.
    let mut swapped = record.clone();
    swapped.structural_returns_mut_for_test().swap(0, 1);
    assert_eq!(
        encode_installation_record(&swapped),
        Err(invalid_affine.clone())
    );
    let mut duplicated = record.clone();
    let row = duplicated.structural_returns()[0].clone();
    duplicated.structural_returns_mut_for_test().insert(1, row);
    assert_eq!(
        encode_installation_record(&duplicated),
        Err(invalid_affine.clone())
    );
    let mut dropped = record.clone();
    dropped.structural_returns_mut_for_test().pop();
    let bytes = encode_installation_record(&dropped).expect("dropped row encodes");
    let replayed = decode_installation_record(&bytes).expect("dropped row decodes");
    assert_eq!(replayed, dropped);
    assert_ne!(
        installation_fingerprint(&replayed).expect("substituted fingerprint"),
        authentic_fingerprint
    );
    assert_eq!(
        validate_installation_record(&replayed, &image),
        Err(InstallationError::ImageBindingMismatch),
        "dropped row: independent replay rejects the thinned roster"
    );
}

#[test]
fn installation_decoder_rejects_alternate_and_malformed_encodings() {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("image");
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
