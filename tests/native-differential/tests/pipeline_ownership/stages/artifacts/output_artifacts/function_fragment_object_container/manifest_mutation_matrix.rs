//! Reauthenticated relocation-free object-container manifest and receipt mutations.

use crate::FunctionFragmentReplayInputs;
use crate::tests::*;
use omega_object_file::ObjectLocalSymbolId;
use omega_optimization_core::{
    FunctionFragmentTextSectionManifestIdentity, OptimizationSelectionIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
use omega_target::{Architecture, ObjectFormat};
use psi_core::FuelScheduleIdentity;
use psi_terminal::SemanticFingerprint;

type ManifestMutation = fn(&mut FunctionFragmentObjectContainerManifest);

fn staged_object_container() -> StagedOptimizedRelocationFreeObjectContainer {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let realization = (physical)
        .into_post_allocation_machine_for_test()
        .unwrap_or_else(|| panic!("CBNZ must complete its direct post-allocation realization"));
    let fragments = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::PostAllocationMachine(Box::new(realization)).into(),
    )
    .unwrap();
    let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
    stage_optimized_relocation_free_object_container(text).unwrap()
}

#[test]
fn current_object_data_outlives_custody_and_replay_rejects_rehashed_substitution() {
    let staged = staged_object_container();
    let object = staged.shared_object();
    let container = staged.shared_container();
    let manifest = staged.manifest().shared_record();
    assert!(std::sync::Arc::ptr_eq(&object, &staged.shared_object()));
    assert!(std::sync::Arc::ptr_eq(
        &container,
        &staged.shared_container()
    ));
    assert!(std::sync::Arc::ptr_eq(
        &manifest,
        &staged.manifest().shared_record()
    ));
    let text = staged.source().text_section().clone();
    let selections = object.selections;
    drop(staged);
    omega_object_file::validate_relocation_free_object_from_text(&text, selections, &object)
        .expect("current data remains independently checkable without its producer");
    assert_eq!(
        omega_object_file::decode_relocation_free_object(&container.bytes).unwrap(),
        *object,
    );
    assert_eq!(manifest.object, object.identity);

    let mutations: [fn(&mut omega_object_file::RelocationFreeObjectPlan); 5] = [
        |object| object.text_section.bytes[0] ^= 1,
        |object| object.psi.program_fingerprint = SemanticFingerprint::from_bytes([77; 32]),
        |object| object.fuel_schedule = FuelScheduleIdentity::new(90_001).unwrap(),
        |object| object.selections = OptimizationSelectionIdentity::from_bytes([78; 32]),
        |object| {
            object.source_text_section =
                TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                    b"substituted source",
                )
        },
    ];
    for mutate in mutations {
        let mut changed = (*object).clone();
        mutate(&mut changed);
        changed.identity = changed.recomputed_identity().unwrap();
        omega_object_file::validate_relocation_free_object(&changed)
            .expect("mutation is a canonical object, not a stale-hash test");
        assert_eq!(
            omega_object_file::validate_relocation_free_object_from_text(
                &text, selections, &changed
            ),
            Err(omega_object_file::RelocationFreeObjectFromTextError::SourceMismatch),
        );
    }
}

#[test]
fn every_representable_object_container_manifest_field_rejects_after_reauthentication() {
    let mut staged = staged_object_container();
    let baseline = staged.manifest().record().clone();
    // Stage, vocabulary, symbol policy, relocation requirement, and unavailable-
    // data values are singleton in memory; the wire matrix rejects their tags.
    let mutations: [(&str, ManifestMutation); 21] = [
        ("source_text_section_manifest", |record| {
            record.source_text_section_manifest =
                FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(
                    b"other source text-section manifest",
                )
        }),
        ("text_section", |record| {
            record.text_section = TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                b"other text section",
            )
        }),
        ("psi.program_fingerprint", |record| {
            record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0x91; 32])
        }),
        ("fuel_schedule", |record| {
            record.fuel_schedule = FuelScheduleIdentity::new(9_901).unwrap()
        }),
        ("selections", |record| {
            record.selections = OptimizationSelectionIdentity::from_bytes([0x92; 32])
        }),
        ("selected", |record| {
            record.selected =
                omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0x93; 32])
        }),
        ("target.architecture", |record| {
            record.target.architecture = Architecture::X86_64
        }),
        ("target.object_format", |record| {
            record.target.object_format = ObjectFormat::MachO
        }),
        ("target.pointer_size", |record| {
            record.target.pointer_size += 1
        }),
        ("target.pointer_alignment", |record| {
            record.target.pointer_alignment += 1
        }),
        ("semantic_entry", |record| {
            record.semantic_entry = MachineId::new(99_901).unwrap()
        }),
        ("semantic_entry_symbol", |record| {
            record.semantic_entry_symbol = ObjectLocalSymbolId::new(99_902).unwrap()
        }),
        ("object", |record| {
            record.object = RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"other object")
        }),
        ("object_container", |record| {
            record.object_container =
                RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"other container")
        }),
        ("statistics.sections", |record| {
            record.statistics.sections += 1
        }),
        ("statistics.function_symbols", |record| {
            record.statistics.function_symbols += 1
        }),
        ("statistics.object_local_symbols", |record| {
            record.statistics.object_local_symbols += 1
        }),
        ("statistics.external_symbols", |record| {
            record.statistics.external_symbols += 1
        }),
        ("statistics.text_bytes", |record| {
            record.statistics.text_bytes += 1
        }),
        ("statistics.container_bytes", |record| {
            record.statistics.container_bytes += 1
        }),
        ("statistics.relocation_records", |record| {
            record.statistics.relocation_records += 1
        }),
    ];

    for (field, mutate) in mutations {
        *staged.manifest_mut().record_mut() = baseline.clone();
        let record = staged.manifest_mut().record_mut();
        mutate(record);
        record.identity = record.recomputed_identity();
        assert_eq!(
            validate_optimized_relocation_free_object_container(&staged),
            Err(RelocationFreeObjectContainerError::ManifestMismatch),
            "reauthenticated {field} mutation must fail independent replay",
        );
    }

    *staged.manifest_mut().record_mut() = baseline;
    staged.manifest_mut().record_mut().identity =
        omega_optimization_core::FunctionFragmentObjectContainerManifestIdentity::from_bytes(
            [0x94; 32],
        );
    assert_eq!(
        validate_optimized_relocation_free_object_container(&staged),
        Err(RelocationFreeObjectContainerError::ManifestMismatch),
    );
}

#[test]
fn object_container_manifest_wire_rejects_every_closed_tag_and_envelope_mutation() {
    let staged = staged_object_container();
    let encoded = staged.manifest().record().encode();
    assert_eq!(encoded.len(), 371, "relocation-free V1 layout is pinned");

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&wrong_magic),
        Err(FunctionFragmentObjectContainerManifestDecodeError::WrongMagic),
    );

    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&99_u32.to_le_bytes());
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&wrong_version),
        Err(FunctionFragmentObjectContainerManifestDecodeError::UnsupportedVersion(99)),
    );

    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&wrong_identity),
        Err(FunctionFragmentObjectContainerManifestDecodeError::IdentityMismatch),
    );

    let mut unknown_stage = encoded.clone();
    unknown_stage[44] = 99;
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&unknown_stage),
        Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownStage(99)),
    );

    let mut unknown_vocabulary = encoded.clone();
    unknown_vocabulary[109..111].copy_from_slice(&59_u16.to_le_bytes());
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&unknown_vocabulary),
        Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownVocabulary(59)),
    );

    let mut invalid_fuel = encoded.clone();
    invalid_fuel[143..147].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&invalid_fuel),
        Err(FunctionFragmentObjectContainerManifestDecodeError::InvalidFuelSchedule),
    );

    let mut unknown_architecture = encoded.clone();
    unknown_architecture[211] = 99;
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&unknown_architecture),
        Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownArchitecture(99)),
    );

    let mut unknown_object_format = encoded.clone();
    unknown_object_format[212] = 99;
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&unknown_object_format),
        Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownObjectFormat(99)),
    );

    let mut invalid_semantic_entry = encoded.clone();
    invalid_semantic_entry[229..237].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&invalid_semantic_entry),
        Err(FunctionFragmentObjectContainerManifestDecodeError::InvalidSemanticEntry),
    );

    let mut invalid_symbol = encoded.clone();
    invalid_symbol[237..245].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&invalid_symbol),
        Err(FunctionFragmentObjectContainerManifestDecodeError::InvalidSymbolId),
    );

    let mut unknown_symbol_policy = encoded.clone();
    unknown_symbol_policy[245] = 99;
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&unknown_symbol_policy),
        Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownSymbolPolicy),
    );

    let mut unknown_relocations = encoded.clone();
    unknown_relocations[310] = 99;
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&unknown_relocations),
        Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownRelocationRequirements),
    );

    for offset in 367..371 {
        let mut unknown_unavailable = encoded.clone();
        unknown_unavailable[offset] = 99;
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(&unknown_unavailable),
            Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownUnavailableStatus),
            "unavailable field at wire offset {offset} must fail closed",
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&trailing),
        Err(FunctionFragmentObjectContainerManifestDecodeError::TrailingBytes),
    );
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&encoded[..encoded.len() - 1]),
        Err(FunctionFragmentObjectContainerManifestDecodeError::Truncated),
    );
}

#[test]
fn every_object_container_receipt_root_rejects_independently() {
    for mutate in [
        StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_source_text_section_manifest_for_test,
        StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_text_section_for_test,
        StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_object_for_test,
        StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_object_container_for_test,
        StagedOptimizedRelocationFreeObjectContainer::corrupt_custody_manifest_for_test,
    ] {
        let mut staged = staged_object_container();
        mutate(&mut staged);
        assert_eq!(
            validate_optimized_relocation_free_object_container(&staged),
            Err(RelocationFreeObjectContainerError::ReceiptMismatch),
        );
    }
}
