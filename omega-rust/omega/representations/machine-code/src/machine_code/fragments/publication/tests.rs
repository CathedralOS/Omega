use super::*;

#[test]
fn prior_wire_versions_with_route_taxonomy_reject() {
    let encoded = record().encode();
    for version in 0..14_u32 {
        let mut stale = encoded.clone();
        stale[8..12].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&stale),
            Err(FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(version))
        );
    }
}

fn record() -> FunctionFragmentEmissionManifest {
    let unavailable = FunctionFragmentEmissionUnavailableData::Unavailable;
    let mut record = FunctionFragmentEmissionManifest {
        identity: FunctionFragmentEmissionManifestIdentity::from_bytes([0; 32]),
        stage: FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
        source_realization: FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(
            [1; 32],
        ),
        selections: OptimizationSelectionIdentity::from_bytes([2; 32]),
        psi: TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([3; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(4).unwrap(),
        selected: selected_instructions::SelectedInstructionPlanIdentity::from_bytes([5; 32]),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes([6; 32]),
        post_allocation_machine: physical_instructions::PostAllocationMachineIdentity::from_bytes(
            [7; 32],
        ),
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
            unresolved_internal_machine_fixups: 12,
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
fn publication_roundtrips_without_route_taxonomy() {
    let record = record();
    let bytes = record.encode();
    assert_eq!(&bytes[..8], b"OMGFFE\0\0");
    assert_eq!(&bytes[8..12], &14_u32.to_le_bytes());
    assert_eq!(bytes.len(), 459);
    assert_eq!(FunctionFragmentEmissionManifest::decode(&bytes), Ok(record));
}

#[test]
fn publication_codec_checks_integrity_not_the_truth_of_claimed_statistics() {
    let mut record = record();
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
