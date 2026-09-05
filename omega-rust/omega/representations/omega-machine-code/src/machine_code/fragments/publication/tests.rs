use super::*;

fn record(source_kind: FunctionFragmentEmissionSourceKind) -> FunctionFragmentEmissionManifest {
    let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
    let mut record = FunctionFragmentEmissionManifest {
        identity: FunctionFragmentEmissionManifestIdentity::from_bytes([0; 32]),
        stage: FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
        source_kind,
        source_realization: FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(
            [1; 32],
        ),
        selections: OptimizationSelectionIdentity::from_bytes([2; 32]),
        psi: TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([3; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(4).unwrap(),
        selected: omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes([5; 32]),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes([6; 32]),
        post_allocation_machine:
            omega_physical_instructions::PostAllocationMachineIdentity::from_bytes([7; 32]),
        final_pre_layout: SelectedFormEncodingIdentity::from_bytes([8; 32]),
        final_resolved_layout: crate::ResolvedSelectedFormLayoutIdentity::from_bytes([9; 32]),
        whole_function_exit_contract: WholeFunctionExitContractIdentity::from_bytes([10; 32]),
        fragments: FunctionFragmentEmissionIdentity::from_bytes([11; 32]),
        target: NativeTarget::linux_x64(),
        statistics: FunctionFragmentEmissionStatistics {
            functions: 1,
            blocks: 2,
            instruction_spans: 3,
            zero_byte_instruction_spans: 4,
            bytes: 5,
            resolved_conditional_branches: 6,
            logical_fuel_settlements: 7,
            structural_unit_functions: 8,
            structural_unit_blocks: 9,
            structural_unit_instruction_spans: 10,
            structural_unit_bytes: 11,
            unresolved_internal_machine_fixups: 12,
            structural_logical_fuel_settlements: 13,
        },
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    record
}

#[test]
fn publication_records_roundtrip_without_a_compiler_or_admission_capsule() {
    let sources = [
        FunctionFragmentEmissionSourceKind::X86Rel8V1,
        FunctionFragmentEmissionSourceKind::SelectedLoweringV1,
        FunctionFragmentEmissionSourceKind::AllocationRecoveryV1,
        FunctionFragmentEmissionSourceKind::UnitBaselineV1,
        FunctionFragmentEmissionSourceKind::StructuralUnitV1,
        FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
        },
    ];
    for source in sources {
        let record = record(source);
        let encoded = record.encode();
        assert_eq!(&encoded[..8], b"OMGFFE\0\0");
        assert_eq!(&encoded[8..12], &10_u32.to_le_bytes());
        let extra_rule_tag = usize::from(matches!(
            source,
            FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 { .. }
        ));
        assert_eq!(encoded.len(), 500 + extra_rule_tag);
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&encoded),
            Ok(record)
        );
    }
}

#[test]
fn publication_codec_checks_integrity_not_the_truth_of_claimed_statistics() {
    let mut record = record(FunctionFragmentEmissionSourceKind::UnitBaselineV1);
    let mut encoded = record.encode();
    encoded[12] ^= 1;
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&encoded),
        Err(FunctionFragmentEmissionManifestDecodeError::IdentityMismatch),
    );

    // Only replay against admitted fragments can establish these counts.
    record.statistics.bytes = u64::MAX;
    record.identity = record.recomputed_identity();
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&record.encode()),
        Ok(record)
    );
}
