//! Reauthenticated function-fragment manifest and receipt mutation coverage.

use crate::FunctionFragmentReplayInputs;
use crate::tests::*;
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionRelativeOptimizationRealizationManifestIdentity,
    OptimizationSelectionIdentity, PostAllocationOptimizationManifestIdentity,
};
use omega_target::{Architecture, ObjectFormat};
use psi_core::FuelScheduleIdentity;
use psi_terminal::SemanticFingerprint;

type ManifestMutation = fn(&mut FunctionFragmentEmissionManifest);

fn staged_fragment_emission() -> StagedOptimizedFunctionFragmentEmission {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap();
    let realization = (physical)
        .into_selected_lowering_for_test()
        .unwrap_or_else(|| {
            panic!("exact incoming-u12 selection must retain selected-lowering realization")
        });
    stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::SelectedLowering(Box::new(realization)).into(),
    )
    .unwrap()
}

#[test]
fn every_representable_fragment_manifest_field_rejects_after_reauthentication() {
    let mut staged = staged_fragment_emission();
    let baseline = staged.manifest().record().clone();
    let mutations: [(&str, ManifestMutation); 30] = [
        ("stage", |record| {
            record.stage =
                FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1
        }),
        ("source_kind", |record| {
            record.source_kind = FunctionFragmentEmissionSourceKind::UnitBaselineV1
        }),
        ("source_realization", |record| {
            record.source_realization =
                FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
                    b"other fragment source realization",
                )
        }),
        ("selections", |record| {
            record.selections = OptimizationSelectionIdentity::from_bytes([0x71; 32])
        }),
        ("psi.program_fingerprint", |record| {
            record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0x72; 32])
        }),
        ("fuel_schedule", |record| {
            record.fuel_schedule = FuelScheduleIdentity::new(9_701).unwrap()
        }),
        ("selected", |record| {
            record.selected =
                omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0x73; 32])
        }),
        ("post_allocation_manifest", |record| {
            record.post_allocation_manifest =
                PostAllocationOptimizationManifestIdentity::from_canonical_bytes(
                    b"other post-allocation manifest",
                )
        }),
        ("post_allocation_machine", |record| {
            record.post_allocation_machine =
                omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes([0x74; 32])
        }),
        ("final_pre_layout", |record| {
            record.final_pre_layout = SelectedFormEncodingIdentity::from_bytes([0x75; 32])
        }),
        ("final_resolved_layout", |record| {
            record.final_resolved_layout =
                ResolvedSelectedFormLayoutIdentity::from_bytes([0x76; 32])
        }),
        ("whole_function_exit_contract", |record| {
            record.whole_function_exit_contract =
                WholeFunctionExitContractIdentity::from_bytes([0x77; 32])
        }),
        ("fragments", |record| {
            record.fragments =
                FunctionFragmentEmissionIdentity::from_canonical_bytes(b"other fragment emission")
        }),
        ("target.architecture", |record| {
            record.target.architecture = Architecture::Aarch64
        }),
        ("target.object_format", |record| {
            record.target.object_format = ObjectFormat::Coff
        }),
        ("target.pointer_size", |record| {
            record.target.pointer_size += 1
        }),
        ("target.pointer_alignment", |record| {
            record.target.pointer_alignment += 1
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
        ("statistics.resolved_conditional_branches", |record| {
            record.statistics.resolved_conditional_branches += 1
        }),
        ("statistics.logical_fuel_settlements", |record| {
            record.statistics.logical_fuel_settlements += 1
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
        ("statistics.structural_unit_bytes", |record| {
            record.statistics.structural_unit_bytes += 1
        }),
        ("statistics.unresolved_internal_machine_fixups", |record| {
            record.statistics.unresolved_internal_machine_fixups += 1
        }),
        ("statistics.structural_logical_fuel_settlements", |record| {
            record.statistics.structural_logical_fuel_settlements += 1
        }),
    ];

    for (field, mutate) in mutations {
        *staged.manifest_record_mut() = baseline.clone();
        let record = staged.manifest_record_mut();
        mutate(record);
        record.identity = record.recomputed_identity();
        assert_eq!(
            validate_optimized_function_fragment_emission(&staged),
            Err(FunctionFragmentEmissionError::ManifestMismatch),
            "reauthenticated {field} mutation must fail independent replay",
        );
    }

    *staged.manifest_record_mut() = baseline;
    staged.manifest_record_mut().identity =
        omega_optimization_core::FunctionFragmentEmissionManifestIdentity::from_bytes([0x78; 32]);
    assert_eq!(
        validate_optimized_function_fragment_emission(&staged),
        Err(FunctionFragmentEmissionError::ManifestMismatch),
    );
}

#[test]
fn fragment_manifest_wire_rejects_every_closed_tag_and_envelope_mutation() {
    let staged = staged_fragment_emission();
    let encoded = staged.manifest().record().encode();
    assert_eq!(encoded.len(), 500, "selected-lowering V9 layout is pinned");

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&wrong_magic),
        Err(FunctionFragmentEmissionManifestDecodeError::WrongMagic),
    );

    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&99_u32.to_le_bytes());
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&wrong_version),
        Err(FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(99)),
    );

    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&wrong_identity),
        Err(FunctionFragmentEmissionManifestDecodeError::IdentityMismatch),
    );

    let mut unknown_stage = encoded.clone();
    unknown_stage[44] = 99;
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&unknown_stage),
        Err(FunctionFragmentEmissionManifestDecodeError::UnknownStage(
            99
        )),
    );

    let mut unknown_source = encoded.clone();
    unknown_source[45] = 99;
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&unknown_source),
        Err(FunctionFragmentEmissionManifestDecodeError::UnknownSourceKind(99)),
    );

    let mut unknown_machine_rule = encoded.clone();
    unknown_machine_rule[45] = 2;
    unknown_machine_rule.insert(46, 99);
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&unknown_machine_rule),
        Err(
            FunctionFragmentEmissionManifestDecodeError::UnknownPostAllocationMachineOptimization(
                99,
            ),
        ),
    );

    let mut unknown_vocabulary = encoded.clone();
    unknown_vocabulary[110..112].copy_from_slice(&59_u16.to_le_bytes());
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&unknown_vocabulary),
        Err(FunctionFragmentEmissionManifestDecodeError::UnknownVocabulary(59)),
    );

    let mut invalid_fuel = encoded.clone();
    invalid_fuel[144..148].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&invalid_fuel),
        Err(FunctionFragmentEmissionManifestDecodeError::InvalidFuelSchedule),
    );

    let mut unknown_architecture = encoded.clone();
    unknown_architecture[372] = 99;
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&unknown_architecture),
        Err(FunctionFragmentEmissionManifestDecodeError::UnknownArchitecture(99)),
    );

    let mut unknown_object_format = encoded.clone();
    unknown_object_format[373] = 99;
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&unknown_object_format),
        Err(FunctionFragmentEmissionManifestDecodeError::UnknownObjectFormat(99)),
    );

    for offset in encoded.len() - 6..encoded.len() {
        let mut unknown_unavailable = encoded.clone();
        unknown_unavailable[offset] = 99;
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&unknown_unavailable),
            Err(FunctionFragmentEmissionManifestDecodeError::UnknownUnavailableStatus),
            "unavailable field at wire offset {offset} must fail closed",
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&trailing),
        Err(FunctionFragmentEmissionManifestDecodeError::TrailingBytes),
    );
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&encoded[..encoded.len() - 1]),
        Err(FunctionFragmentEmissionManifestDecodeError::Truncated),
    );
}

#[test]
fn every_fragment_emission_receipt_root_rejects_independently() {
    for mutate in [
        StagedOptimizedFunctionFragmentEmission::corrupt_custody_source_realization_for_test,
        StagedOptimizedFunctionFragmentEmission::corrupt_custody_fragments_for_test,
        StagedOptimizedFunctionFragmentEmission::corrupt_custody_manifest_for_test,
    ] {
        let mut staged = staged_fragment_emission();
        mutate(&mut staged);
        assert_eq!(
            validate_optimized_function_fragment_emission(&staged),
            Err(FunctionFragmentEmissionError::ReceiptMismatch),
        );
    }
}
