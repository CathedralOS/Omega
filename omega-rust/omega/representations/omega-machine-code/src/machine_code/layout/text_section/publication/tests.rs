use super::*;

fn record(source_kind: FunctionFragmentEmissionSourceKind) -> FunctionFragmentTextSectionManifest {
    let unavailable = FunctionFragmentTextSectionUnavailableData::Unavailable;
    let mut value = FunctionFragmentTextSectionManifest {
        identity: FunctionFragmentTextSectionManifestIdentity::from_bytes([0; 32]),
        stage: FunctionFragmentTextSectionStage::ValidatedRelocationFreeTextSectionPlacementV1,
        source_custody: FunctionFragmentTextSectionSourceCustody::DirectFragmentEmissionV1,
        source_kind,
        source_fragment_manifest: FunctionFragmentEmissionManifestIdentity::from_bytes([1; 32]),
        source_realization: FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(
            [2; 32],
        ),
        selections: OptimizationSelectionIdentity::from_bytes([3; 32]),
        psi: TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([4; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(5).unwrap(),
        selected: SelectedInstructionPlanIdentity::from_bytes([6; 32]),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes([7; 32]),
        post_allocation_machine:
            omega_physical_instructions::PostAllocationMachineIdentity::from_bytes([8; 32]),
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
    if source_kind == FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1 {
        value.stage =
            FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1;
        value.source_custody = FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 {
            application: FunctionFragmentFrameApplicationIdentity::from_bytes([16; 32]),
        };
    }
    value.identity = value.recomputed_identity();
    value
}

#[test]
fn text_publication_roundtrips_without_a_compiler_or_admission_capsule() {
    for kind in [
        FunctionFragmentEmissionSourceKind::X86Rel8V1,
        FunctionFragmentEmissionSourceKind::SelectedLoweringV1,
        FunctionFragmentEmissionSourceKind::AllocationRecoveryV1,
        FunctionFragmentEmissionSourceKind::UnitBaselineV1,
        FunctionFragmentEmissionSourceKind::StructuralUnitV1,
        FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization:
                omega_optimization_core::Optimization::X86SelectXorZeroI64MaterializationV1,
        },
    ] {
        let record = record(kind);
        let bytes = record.encode();
        assert_eq!(&bytes[..8], b"OMGTSP\0\0");
        assert_eq!(&bytes[8..12], &11_u32.to_le_bytes());
        let extension = match kind {
            FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1 => 32,
            FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 { .. } => 1,
            _ => 0,
        };
        assert_eq!(bytes.len(), 599 + extension);
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&bytes),
            Ok(record)
        );
    }
}

#[test]
fn codec_checks_custody_shape_and_identity_not_the_truth_of_counts() {
    let mut record = record(FunctionFragmentEmissionSourceKind::UnitBaselineV1);
    record.statistics.bytes = u64::MAX;
    record.identity = record.recomputed_identity();
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&record.encode()),
        Ok(record.clone())
    );
    record.stage =
        FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1;
    record.identity = record.recomputed_identity();
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&record.encode()),
        Err(FunctionFragmentTextSectionManifestDecodeError::SourceCustodyMismatch)
    );
}
