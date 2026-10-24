//! Canonical object-artifact and manifest wire encoding.

use super::{
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentObjectContainerManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, MachineId, NativeTarget,
    OptimizationSelectionIdentity, OptimizedObjectArtifactIdentity,
    OptimizedObjectArtifactManifest, OptimizedObjectArtifactRecord,
    OptimizedObjectArtifactRecordDecodeError, OptimizedObjectArtifactStatistics,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity, SemanticFingerprint,
    TerminalPsiIdentity, VocabularyMarker,
};
use crate::cursor::Cursor;
pub(super) fn encode_artifact_content(record: &OptimizedObjectArtifactRecord) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&record.psi_artifact);
    terminal_codec::append_terminal_identity(&mut bytes, record.psi);
    encode_optional_array(&mut bytes, record.debug_section);
    bytes.extend_from_slice(&record.selections.bytes());
    bytes.extend_from_slice(&record.target.wire_identity_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&record.pre_physical_manifest.bytes());
    bytes.extend_from_slice(&record.post_allocation_manifest.bytes());
    bytes.extend_from_slice(&record.function_relative_manifest.bytes());
    bytes.extend_from_slice(&record.function_fragment_manifest.bytes());
    bytes.extend_from_slice(&record.text_section_manifest.bytes());
    bytes.extend_from_slice(&record.object_container_manifest.bytes());
    bytes.extend_from_slice(&record.object.bytes());
    bytes.extend_from_slice(&record.object_container.bytes());
    encode_statistics(&mut bytes, record.statistics);
    bytes
}

pub(super) fn decode_artifact_content(
    cursor: &mut Cursor<'_>,
    identity: OptimizedObjectArtifactIdentity,
) -> Result<OptimizedObjectArtifactRecord, OptimizedObjectArtifactRecordDecodeError> {
    Ok(OptimizedObjectArtifactRecord {
        identity,
        psi_artifact: cursor.array()?,
        psi: decode_psi(cursor)?,
        debug_section: decode_optional_array(cursor)?,
        selections: OptimizationSelectionIdentity::from_bytes(cursor.array()?),
        target: NativeTarget::from_wire_identity_bytes(&cursor.array()?)?,
        semantic_entry: decode_machine(cursor)?,
        pre_physical_manifest: PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?),
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes(
            cursor.array()?,
        ),
        function_relative_manifest:
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?),
        function_fragment_manifest: FunctionFragmentEmissionManifestIdentity::from_bytes(
            cursor.array()?,
        ),
        text_section_manifest: FunctionFragmentTextSectionManifestIdentity::from_bytes(
            cursor.array()?,
        ),
        object_container_manifest: FunctionFragmentObjectContainerManifestIdentity::from_bytes(
            cursor.array()?,
        ),
        object: RelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?),
        object_container: RelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?),
        statistics: decode_statistics(cursor)?,
    })
}

pub(super) fn encode_manifest_content(record: &OptimizedObjectArtifactManifest) -> Vec<u8> {
    let mut bytes = vec![1];
    bytes.extend_from_slice(&record.artifact.bytes());
    bytes.extend_from_slice(&record.psi_artifact);
    terminal_codec::append_terminal_identity(&mut bytes, record.psi);
    bytes.extend_from_slice(&record.selections.bytes());
    bytes.extend_from_slice(&record.target.wire_identity_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&record.object_container_manifest.bytes());
    bytes.extend_from_slice(&record.object.bytes());
    bytes.extend_from_slice(&record.object_container.bytes());
    encode_statistics(&mut bytes, record.statistics);
    bytes.extend_from_slice(&[1; 4]);
    bytes
}

pub(super) fn encode_statistics(
    bytes: &mut Vec<u8>,
    statistics: OptimizedObjectArtifactStatistics,
) {
    crate::codec_bytes::push_encoded(bytes, statistics.text_bytes.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, statistics.object_container_bytes.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, statistics.function_symbols.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, statistics.relocation_records.to_le_bytes());
}

pub(super) fn decode_statistics(
    cursor: &mut Cursor<'_>,
) -> Result<OptimizedObjectArtifactStatistics, OptimizedObjectArtifactRecordDecodeError> {
    Ok(OptimizedObjectArtifactStatistics {
        text_bytes: cursor.u64()?,
        object_container_bytes: cursor.u64()?,
        function_symbols: cursor.u64()?,
        relocation_records: cursor.u64()?,
    })
}

pub(super) fn decode_psi(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, OptimizedObjectArtifactRecordDecodeError> {
    let marker = cursor.u16()?;
    Ok(TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::new(marker).ok_or(
            OptimizedObjectArtifactRecordDecodeError::UnknownVocabulary(marker),
        )?,
        program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
    })
}

pub(super) fn decode_machine(
    cursor: &mut Cursor<'_>,
) -> Result<MachineId, OptimizedObjectArtifactRecordDecodeError> {
    MachineId::new(cursor.u64()?).ok_or(OptimizedObjectArtifactRecordDecodeError::InvalidMachine)
}

pub(super) fn encode_optional_array(bytes: &mut Vec<u8>, value: Option<[u8; 32]>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value);
        }
    }
}

pub(super) fn decode_optional_array(
    cursor: &mut Cursor<'_>,
) -> Result<Option<[u8; 32]>, OptimizedObjectArtifactRecordDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.array()?)),
        tag => Err(OptimizedObjectArtifactRecordDecodeError::UnknownOptionalTag(tag)),
    }
}
