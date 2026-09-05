//! Object-container manifest codec tests.

use super::*;

#[test]
fn manifest_codec_rejects_wrong_magic_version_and_trailing_bytes() {
    let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
    let mut manifest = FunctionFragmentObjectContainerManifest {
        identity: FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionFragmentObjectContainerStage::ValidatedRelocationFreeObjectContainerV1,
        source_text_section_manifest:
            FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"source"),
        text_section: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"text"),
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        selections: OptimizationSelectionIdentity::from_bytes([2; 32]),
        selected: SelectedInstructionPlanIdentity::from_bytes([3; 32]),
        target: NativeTarget::linux_x64(),
        semantic_entry: MachineId::new(1).unwrap(),
        semantic_entry_symbol: ObjectLocalSymbolId::new(1).unwrap(),
        symbol_policy: RelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
        object: RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"object"),
        object_container: RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"container"),
        relocation_requirements:
            RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        statistics: FunctionFragmentObjectContainerStatistics {
            sections: 1,
            function_symbols: 1,
            object_local_symbols: 1,
            external_symbols: 0,
            text_bytes: 3,
            container_bytes: 10,
            relocation_records: 0,
        },
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    manifest.identity = manifest.recomputed_identity();
    let encoded = manifest.encode();
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&encoded),
        Ok(manifest)
    );
    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&wrong_magic),
        Err(FunctionFragmentObjectContainerManifestDecodeError::WrongMagic)
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8] = 2;
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&wrong_version),
        Err(FunctionFragmentObjectContainerManifestDecodeError::UnsupportedVersion(2))
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&trailing),
        Err(FunctionFragmentObjectContainerManifestDecodeError::TrailingBytes)
    );
}
