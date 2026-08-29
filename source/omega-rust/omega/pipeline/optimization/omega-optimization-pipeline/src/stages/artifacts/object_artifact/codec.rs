//! Canonical object-artifact and manifest wire encoding.

use super::*;

pub(super) fn encode_artifact_content(record: &OptimizedObjectArtifactRecord) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&record.psi_artifact);
    encode_psi(&mut bytes, record.psi);
    bytes.extend_from_slice(&record.obligation_ledger);
    bytes.extend_from_slice(&record.proof_bundle);
    encode_optional_array(&mut bytes, record.debug_section);
    bytes.extend_from_slice(&record.selections.bytes());
    encode_target(&mut bytes, record.target);
    bytes.extend_from_slice(&record.semantic_entry.get().to_le_bytes());
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
        obligation_ledger: cursor.array()?,
        proof_bundle: cursor.array()?,
        debug_section: decode_optional_array(cursor)?,
        selections: OptimizationSelectionIdentity::from_bytes(cursor.array()?),
        target: decode_target(cursor)?,
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
    encode_psi(&mut bytes, record.psi);
    bytes.extend_from_slice(&record.selections.bytes());
    encode_target(&mut bytes, record.target);
    bytes.extend_from_slice(&record.semantic_entry.get().to_le_bytes());
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
    bytes.extend_from_slice(&statistics.text_bytes.to_le_bytes());
    bytes.extend_from_slice(&statistics.object_container_bytes.to_le_bytes());
    bytes.extend_from_slice(&statistics.function_symbols.to_le_bytes());
    bytes.extend_from_slice(&statistics.relocation_records.to_le_bytes());
}

pub(super) fn decode_statistics(
    cursor: &mut Cursor<'_>,
) -> Result<OptimizedObjectArtifactStatistics, OptimizedObjectArtifactRecordDecodeError> {
    Ok(OptimizedObjectArtifactStatistics {
        text_bytes: u64::from_le_bytes(cursor.array()?),
        object_container_bytes: u64::from_le_bytes(cursor.array()?),
        function_symbols: u64::from_le_bytes(cursor.array()?),
        relocation_records: u64::from_le_bytes(cursor.array()?),
    })
}

pub(super) fn encode_psi(bytes: &mut Vec<u8>, identity: TerminalPsiIdentity) {
    bytes.extend_from_slice(&identity.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(identity.program_fingerprint.as_bytes());
}

pub(super) fn decode_psi(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, OptimizedObjectArtifactRecordDecodeError> {
    let marker = u16::from_le_bytes(cursor.array()?);
    Ok(TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::new(marker).ok_or(
            OptimizedObjectArtifactRecordDecodeError::UnknownVocabulary(marker),
        )?,
        program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
    })
}

pub(super) fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

pub(super) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, OptimizedObjectArtifactRecordDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(OptimizedObjectArtifactRecordDecodeError::UnknownArchitecture(tag));
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(OptimizedObjectArtifactRecordDecodeError::UnknownObjectFormat(tag));
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| OptimizedObjectArtifactRecordDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| OptimizedObjectArtifactRecordDecodeError::TargetLayoutOverflow)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

pub(super) fn decode_machine(
    cursor: &mut Cursor<'_>,
) -> Result<MachineId, OptimizedObjectArtifactRecordDecodeError> {
    MachineId::new(u64::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedObjectArtifactRecordDecodeError::InvalidMachine)
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

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    pub(super) fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], OptimizedObjectArtifactRecordDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(OptimizedObjectArtifactRecordDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(OptimizedObjectArtifactRecordDecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    pub(super) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], OptimizedObjectArtifactRecordDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OptimizedObjectArtifactRecordDecodeError::Truncated)
    }

    pub(super) fn byte(&mut self) -> Result<u8, OptimizedObjectArtifactRecordDecodeError> {
        Ok(self.array::<1>()?[0])
    }
}
