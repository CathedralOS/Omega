use super::*;

#[test]
fn prior_wire_versions_with_route_taxonomy_reject() {
    let encoded = record().encode();
    for version in 0..16_u32 {
        let mut stale = encoded.clone();
        stale[8..12].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&stale),
            Err(FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(version))
        );
    }
}

fn record() -> FunctionFragmentTextSectionManifest {
    let unavailable = FunctionFragmentTextSectionUnavailableData::Unavailable;
    let mut value = FunctionFragmentTextSectionManifest {
        identity: FunctionFragmentTextSectionManifestIdentity::from_bytes([0; 32]),
        stage:
            FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1,
        frame_application: FunctionFragmentFrameApplicationIdentity::from_bytes([16; 32]),
        source_fragment_manifest: FunctionFragmentEmissionManifestIdentity::from_bytes([1; 32]),
        source_realization: FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(
            [2; 32],
        ),
        selections: OptimizationSelectionIdentity::from_bytes([3; 32]),
        psi: TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([4; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(5).unwrap(),
        selected: SelectedInstructionPlanIdentity::from_bytes([6; 32]),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes([7; 32]),
        post_allocation_machine: physical_instructions::PostAllocationMachineIdentity::from_bytes(
            [8; 32],
        ),
        final_pre_layout: SelectedFormEncodingIdentity::from_bytes([9; 32]),
        final_resolved_layout: ResolvedSelectedFormLayoutIdentity::from_bytes([10; 32]),
        whole_function_exit_contract: WholeFunctionExitContractIdentity::from_bytes([11; 32]),
        fragments: FunctionFragmentEmissionIdentity::from_bytes([12; 32]),
        target: NativeTarget::linux_x64(),
        semantic_entry: MachineId::new(13).unwrap(),
        semantic_entry_offset: 14,
        placement_policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
        text_section: TerminalRelocationFreeTextSectionIdentity::from_bytes([15; 32]),
        relocation_requirements:
            TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        statistics: FunctionFragmentTextSectionStatistics::default(),
        symbols: unavailable,
        object_container: unavailable,
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    value.identity = value.recomputed_identity();
    value
}

#[test]
fn publication_roundtrips_without_route_taxonomy() {
    let record = record();
    let bytes = record.encode();
    assert_eq!(&bytes[..8], b"OMGTSP\0\0");
    assert_eq!(&bytes[8..12], &16_u32.to_le_bytes());
    assert_eq!(bytes.len(), 589);
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&bytes),
        Ok(record)
    );
}

#[test]
fn codec_checks_frame_identity_and_rejects_retired_direct_stage() {
    let mut record = record();
    record.statistics.bytes = u64::MAX;
    record.identity = record.recomputed_identity();
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&record.encode()),
        Ok(record.clone())
    );
    let mut retired = record.encode();
    retired[44] = 1;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&retired),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownStage(1))
    );
}
