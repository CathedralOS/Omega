//! Reauthenticated relocation-free text-section manifest and receipt mutations.

use crate::FunctionFragmentReplayInputs;
use crate::tests::*;
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use omega_target::{Architecture, ObjectFormat};
use psi_core::FuelScheduleIdentity;
use psi_terminal::SemanticFingerprint;

type ManifestMutation = fn(&mut FunctionFragmentTextSectionManifest);

fn staged_text_section() -> StagedOptimizedRelocationFreeTextSection {
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
    stage_optimized_relocation_free_text_section(fragments).unwrap()
}

#[test]
fn every_representable_text_manifest_field_rejects_after_reauthentication() {
    let mut staged = staged_text_section();
    let baseline = staged.manifest().record().clone();
    // Vocabulary, placement, relocation, and unavailable-data values are
    // singleton in memory; the wire matrix below rejects alternate tags.
    let mutations: [(&str, ManifestMutation); 38] = [
        ("stage", |record| {
            record.stage = FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1
        }),
        ("source_custody", |record| {
            record.source_custody =
                FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 {
                    application: FunctionFragmentFrameApplicationIdentity::from_bytes([0x80; 32]),
                }
        }),
        ("source_kind", |record| {
            record.source_kind = FunctionFragmentEmissionSourceKind::UnitBaselineV1
        }),
        ("source_kind.optimization", |record| {
            record.source_kind =
                FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                    optimization: Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                }
        }),
        ("source_fragment_manifest", |record| {
            record.source_fragment_manifest =
                FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(
                    b"other source fragment manifest",
                )
        }),
        ("source_realization", |record| {
            record.source_realization =
                FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
                    b"other source realization",
                )
        }),
        ("selections", |record| {
            record.selections = OptimizationSelectionIdentity::from_bytes([0x81; 32])
        }),
        ("psi.program_fingerprint", |record| {
            record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0x82; 32])
        }),
        ("fuel_schedule", |record| {
            record.fuel_schedule = FuelScheduleIdentity::new(9_801).unwrap()
        }),
        ("selected", |record| {
            record.selected =
                omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0x83; 32])
        }),
        ("post_allocation_manifest", |record| {
            record.post_allocation_manifest =
                PostAllocationOptimizationManifestIdentity::from_canonical_bytes(
                    b"other post-allocation manifest",
                )
        }),
        ("post_allocation_machine", |record| {
            record.post_allocation_machine =
                omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes([0x84; 32])
        }),
        ("final_pre_layout", |record| {
            record.final_pre_layout = SelectedFormEncodingIdentity::from_bytes([0x85; 32])
        }),
        ("final_resolved_layout", |record| {
            record.final_resolved_layout =
                ResolvedSelectedFormLayoutIdentity::from_bytes([0x86; 32])
        }),
        ("whole_function_exit_contract", |record| {
            record.whole_function_exit_contract =
                WholeFunctionExitContractIdentity::from_bytes([0x87; 32])
        }),
        ("fragments", |record| {
            record.fragments =
                FunctionFragmentEmissionIdentity::from_canonical_bytes(b"other fragments")
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
            record.semantic_entry = MachineId::new(99_801).unwrap()
        }),
        ("semantic_entry_offset", |record| {
            record.semantic_entry_offset += 1
        }),
        ("text_section", |record| {
            record.text_section = TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                b"other text section",
            )
        }),
        ("statistics.functions", |record| {
            record.statistics.functions += 1
        }),
        ("statistics.blocks", |record| record.statistics.blocks += 1),
        ("statistics.instruction_spans", |record| {
            record.statistics.instruction_spans += 1
        }),
        ("statistics.zero_byte_instruction_spans", |record| {
            record.statistics.zero_byte_instruction_spans += 1
        }),
        ("statistics.bytes", |record| record.statistics.bytes += 1),
        ("statistics.padding_bytes", |record| {
            record.statistics.padding_bytes += 1
        }),
        ("statistics.relocation_requirements", |record| {
            record.statistics.relocation_requirements += 1
        }),
        ("statistics.structural_unit_functions", |record| {
            record.statistics.structural_unit_functions += 1
        }),
        ("statistics.structural_unit_blocks", |record| {
            record.statistics.structural_unit_blocks += 1
        }),
        ("statistics.structural_unit_instruction_spans", |record| {
            record.statistics.structural_unit_instruction_spans += 1
        }),
        (
            "statistics.structural_unit_zero_byte_instruction_spans",
            |record| {
                record
                    .statistics
                    .structural_unit_zero_byte_instruction_spans += 1
            },
        ),
        ("statistics.structural_unit_bytes", |record| {
            record.statistics.structural_unit_bytes += 1
        }),
        ("statistics.source_internal_machine_fixups", |record| {
            record.statistics.source_internal_machine_fixups += 1
        }),
        ("statistics.resolved_internal_machine_fixups", |record| {
            record.statistics.resolved_internal_machine_fixups += 1
        }),
        ("statistics.remaining_internal_machine_fixups", |record| {
            record.statistics.remaining_internal_machine_fixups += 1
        }),
    ];

    for (field, mutate) in mutations {
        *staged.manifest_mut().record_mut() = baseline.clone();
        let record = staged.manifest_mut().record_mut();
        mutate(record);
        record.identity = record.recomputed_identity();
        if field != "stage" && field != "source_custody" {
            assert_eq!(
                FunctionFragmentTextSectionManifest::decode(&record.encode()),
                Ok(record.clone()),
                "canonical {field} claim decodes as data, not as admission"
            );
        }
        assert_eq!(
            validate_optimized_relocation_free_text_section(&staged),
            Err(RelocationFreeTextSectionPlacementError::ManifestMismatch),
            "reauthenticated {field} mutation must fail independent replay",
        );
    }

    *staged.manifest_mut().record_mut() = baseline;
    staged.manifest_mut().record_mut().identity =
        omega_optimization_core::FunctionFragmentTextSectionManifestIdentity::from_bytes(
            [0x88; 32],
        );
    assert_eq!(
        validate_optimized_relocation_free_text_section(&staged),
        Err(RelocationFreeTextSectionPlacementError::ManifestMismatch),
    );
}

#[test]
fn text_manifest_wire_rejects_every_closed_tag_and_envelope_mutation() {
    let staged = staged_text_section();
    let encoded = staged.manifest().record().encode();
    assert_eq!(encoded.len(), 600, "post-allocation V11 layout is pinned");

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&wrong_magic),
        Err(FunctionFragmentTextSectionManifestDecodeError::WrongMagic),
    );

    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&99_u32.to_le_bytes());
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&wrong_version),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(99)),
    );

    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&wrong_identity),
        Err(FunctionFragmentTextSectionManifestDecodeError::IdentityMismatch),
    );

    let mut unknown_stage = encoded.clone();
    unknown_stage[44] = 99;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_stage),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownStage(99)),
    );

    let mut unknown_source_custody = encoded.clone();
    unknown_source_custody[45] = 99;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_source_custody),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownSourceCustody(99)),
    );

    let mut unknown_source = encoded.clone();
    unknown_source[46] = 99;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_source),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownSourceKind(99)),
    );

    let mut mismatched_custody = staged.manifest().record().clone();
    mismatched_custody.source_custody =
        FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 {
            application: FunctionFragmentFrameApplicationIdentity::from_bytes([0x91; 32]),
        };
    mismatched_custody.identity = mismatched_custody.recomputed_identity();
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&mismatched_custody.encode()),
        Err(FunctionFragmentTextSectionManifestDecodeError::SourceCustodyMismatch),
    );

    let mut unknown_machine_rule = encoded.clone();
    unknown_machine_rule[47] = 99;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_machine_rule),
        Err(
            FunctionFragmentTextSectionManifestDecodeError::UnknownPostAllocationMachineOptimization(
                99,
            ),
        ),
    );

    let mut unknown_vocabulary = encoded.clone();
    unknown_vocabulary[144..146].copy_from_slice(&59_u16.to_le_bytes());
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_vocabulary),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownVocabulary(59)),
    );

    let mut invalid_fuel = encoded.clone();
    invalid_fuel[178..182].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&invalid_fuel),
        Err(FunctionFragmentTextSectionManifestDecodeError::InvalidFuelSchedule),
    );

    let mut unknown_architecture = encoded.clone();
    unknown_architecture[406] = 99;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_architecture),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownArchitecture(99)),
    );

    let mut unknown_object_format = encoded.clone();
    unknown_object_format[407] = 99;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_object_format),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownObjectFormat(99)),
    );

    let mut invalid_semantic_entry = encoded.clone();
    invalid_semantic_entry[424..432].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&invalid_semantic_entry),
        Err(FunctionFragmentTextSectionManifestDecodeError::InvalidSemanticEntry),
    );

    let mut unknown_placement = encoded.clone();
    unknown_placement[440] = 99;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_placement),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownPlacementPolicy(99)),
    );

    let mut unknown_relocations = encoded.clone();
    unknown_relocations[473] = 99;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_relocations),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownRelocationRequirements(99),),
    );

    for offset in 594..600 {
        let mut unknown_unavailable = encoded.clone();
        unknown_unavailable[offset] = 99;
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&unknown_unavailable),
            Err(FunctionFragmentTextSectionManifestDecodeError::UnknownUnavailableStatus),
            "unavailable field at wire offset {offset} must fail closed",
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&trailing),
        Err(FunctionFragmentTextSectionManifestDecodeError::TrailingBytes),
    );
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&encoded[..encoded.len() - 1]),
        Err(FunctionFragmentTextSectionManifestDecodeError::Truncated),
    );
}

#[test]
fn every_text_section_receipt_root_rejects_independently() {
    for mutate in [
        StagedOptimizedRelocationFreeTextSection::corrupt_custody_source_fragment_manifest_for_test,
        StagedOptimizedRelocationFreeTextSection::corrupt_custody_fragments_for_test,
        StagedOptimizedRelocationFreeTextSection::corrupt_custody_text_section_for_test,
        StagedOptimizedRelocationFreeTextSection::corrupt_custody_manifest_for_test,
    ] {
        let mut staged = staged_text_section();
        mutate(&mut staged);
        assert_eq!(
            validate_optimized_relocation_free_text_section(&staged),
            Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch),
        );
    }
}
