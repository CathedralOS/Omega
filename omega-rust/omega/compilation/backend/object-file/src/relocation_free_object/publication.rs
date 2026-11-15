/*
We define the object-container publication manifest
(`FunctionFragmentObjectContainerManifest`), its summary statistics
(`FunctionFragmentObjectContainerStatistics`), and its canonical version-1
`OMGTOM` wire codec. These records capture object-format publication claims and
record counts without granting source admission or executable publication
authority.
*/

use crate::cursor::Cursor;
use crate::{
    ObjectLocalSymbolId, RelocationFreeObjectContainer, RelocationFreeObjectError,
    RelocationFreeObjectPlan, RelocationFreeObjectRelocationRequirements,
    RelocationFreeObjectSymbolPolicy,
};
use optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    OptimizationSelectionIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::{NativeTarget, TargetWireIdentityError};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGTOM\0\0";
const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerStage {
    ValidatedRelocationFreeObjectContainerV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentObjectContainerStatistics {
    pub sections: u64,
    pub function_symbols: u64,
    pub object_local_symbols: u64,
    pub external_symbols: u64,
    pub text_bytes: u64,
    pub container_bytes: u64,
    pub relocation_records: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentObjectContainerManifest {
    pub identity: FunctionFragmentObjectContainerManifestIdentity,
    pub stage: FunctionFragmentObjectContainerStage,
    pub source_text_section_manifest: FunctionFragmentTextSectionManifestIdentity,
    pub text_section: TerminalRelocationFreeTextSectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: ObjectLocalSymbolId,
    pub symbol_policy: RelocationFreeObjectSymbolPolicy,
    pub object: RelocationFreeObjectPlanIdentity,
    pub object_container: RelocationFreeObjectContainerIdentity,
    pub relocation_requirements: RelocationFreeObjectRelocationRequirements,
    pub statistics: FunctionFragmentObjectContainerStatistics,
    pub external_entry_bridge: FunctionFragmentObjectContainerUnavailableData,
    pub executable_image: FunctionFragmentObjectContainerUnavailableData,
    pub installation: FunctionFragmentObjectContainerUnavailableData,
    pub publication: FunctionFragmentObjectContainerUnavailableData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownVocabulary(u16),
    InvalidWorkSchedule,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    InvalidSemanticEntry,
    InvalidSymbolId,
    UnknownSymbolPolicy,
    UnknownRelocationRequirements,
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}

optimization_core::truncated_from_adapter!(FunctionFragmentObjectContainerManifestDecodeError);

impl From<TargetWireIdentityError> for FunctionFragmentObjectContainerManifestDecodeError {
    fn from(error: TargetWireIdentityError) -> Self {
        match error {
            TargetWireIdentityError::UnknownArchitecture(tag) => Self::UnknownArchitecture(tag),
            TargetWireIdentityError::UnknownObjectFormat(tag) => Self::UnknownObjectFormat(tag),
            TargetWireIdentityError::LayoutOverflow => Self::TargetLayoutOverflow,
        }
    }
}

pub(crate) fn relocation_free_object_statistics(
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> Result<FunctionFragmentObjectContainerStatistics, RelocationFreeObjectError> {
    let symbols = u64::try_from(object.symbols.len())
        .map_err(|_| RelocationFreeObjectError::LengthOverflow)?;
    let container_bytes = u64::try_from(container.bytes.len())
        .map_err(|_| RelocationFreeObjectError::LengthOverflow)?;
    Ok(FunctionFragmentObjectContainerStatistics {
        sections: 1,
        function_symbols: symbols,
        object_local_symbols: symbols,
        external_symbols: u64::try_from(object.normalized_imports.len())
            .map_err(|_| RelocationFreeObjectError::LengthOverflow)?,
        text_bytes: object.text_section.byte_count,
        container_bytes,
        relocation_records: object.relocation_record_count,
    })
}

impl FunctionFragmentObjectContainerManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentObjectContainerManifestIdentity {
        let mut canonical = b"omega.function-fragment-object-container-manifest.v1\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        crate::names::encode_manifest_envelope(
            MANIFEST_MAGIC,
            MANIFEST_VERSION,
            &self.identity.bytes(),
            &encode_manifest_content(self),
        )
    }

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, FunctionFragmentObjectContainerManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if version != MANIFEST_VERSION {
            return Err(
                FunctionFragmentObjectContainerManifestDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = FunctionFragmentObjectContainerManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => FunctionFragmentObjectContainerStage::ValidatedRelocationFreeObjectContainerV1,
            tag => {
                return Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownStage(tag));
            }
        };
        let source_text_section_manifest =
            FunctionFragmentTextSectionManifestIdentity::from_bytes(cursor.array()?);
        let text_section = TerminalRelocationFreeTextSectionIdentity::from_bytes(cursor.array()?);
        let marker = cursor.u16()?;
        let vocabulary_marker = VocabularyMarker::new(marker)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::UnknownVocabulary(marker))?;
        let psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        };
        let work = cursor.u32()?;
        let work_schedule = WorkScheduleIdentity::new(work)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidWorkSchedule)?;
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let target = NativeTarget::from_wire_identity_bytes(&cursor.array()?)?;
        let semantic_entry = MachineId::new(cursor.u64()?)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidSemanticEntry)?;
        let semantic_entry_symbol = ObjectLocalSymbolId::new(cursor.u64()?)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidSymbolId)?;
        if cursor.byte()? != 1 {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownSymbolPolicy);
        }
        let object = RelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
        let object_container = RelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
        let relocation_requirements = match cursor.byte()? {
            1 => RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
            2 => RelocationFreeObjectRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1,
            3 => RelocationFreeObjectRelocationRequirements::UnresolvedObjectSymbolFieldsV1,
            _ => {
                return Err(
                    FunctionFragmentObjectContainerManifestDecodeError::UnknownRelocationRequirements,
                );
            }
        };
        let statistics = FunctionFragmentObjectContainerStatistics {
            sections: cursor.u64()?,
            function_symbols: cursor.u64()?,
            object_local_symbols: cursor.u64()?,
            external_symbols: cursor.u64()?,
            text_bytes: cursor.u64()?,
            container_bytes: cursor.u64()?,
            relocation_records: cursor.u64()?,
        };
        for _ in 0..4 {
            if cursor.byte()? != 1 {
                return Err(
                    FunctionFragmentObjectContainerManifestDecodeError::UnknownUnavailableStatus,
                );
            }
        }
        if cursor.remaining() != 0 {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::TrailingBytes);
        }
        let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage,
            source_text_section_manifest,
            text_section,
            psi,
            work_schedule,
            selections,
            selected,
            target,
            semantic_entry,
            semantic_entry_symbol,
            symbol_policy: RelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
            object,
            object_container,
            relocation_requirements,
            statistics,
            external_entry_bridge: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if manifest.recomputed_identity() != identity {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

fn encode_manifest_content(record: &FunctionFragmentObjectContainerManifest) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(1);
    bytes.extend_from_slice(&record.source_text_section_manifest.bytes());
    bytes.extend_from_slice(&record.text_section.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(record.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.work_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&record.selections.bytes());
    bytes.extend_from_slice(&record.selected.bytes());
    bytes.extend_from_slice(&record.target.wire_identity_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.semantic_entry.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.semantic_entry_symbol.get().to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&record.object.bytes());
    bytes.extend_from_slice(&record.object_container.bytes());
    bytes.push(match record.relocation_requirements {
        RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => {
            1
        }
        RelocationFreeObjectRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1 => 2,
        RelocationFreeObjectRelocationRequirements::UnresolvedObjectSymbolFieldsV1 => 3,
    });
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.sections.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.function_symbols.to_le_bytes());
    crate::codec_bytes::push_encoded(
        &mut bytes,
        record.statistics.object_local_symbols.to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.external_symbols.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.text_bytes.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.statistics.container_bytes.to_le_bytes());
    crate::codec_bytes::push_encoded(
        &mut bytes,
        record.statistics.relocation_records.to_le_bytes(),
    );
    bytes.extend_from_slice(&[1; 4]);
    bytes
}

#[cfg(test)]
mod tests {
    use super::{
        FunctionFragmentObjectContainerManifest,
        FunctionFragmentObjectContainerManifestDecodeError,
        FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentObjectContainerStage,
        FunctionFragmentObjectContainerStatistics, FunctionFragmentObjectContainerUnavailableData,
        FunctionFragmentTextSectionManifestIdentity, MachineId, NativeTarget, ObjectLocalSymbolId,
        OptimizationSelectionIdentity, RelocationFreeObjectContainerIdentity,
        RelocationFreeObjectPlanIdentity, RelocationFreeObjectRelocationRequirements,
        RelocationFreeObjectSymbolPolicy, SelectedInstructionPlanIdentity, SemanticFingerprint,
        TerminalPsiIdentity, TerminalRelocationFreeTextSectionIdentity, VocabularyMarker,
        WorkScheduleIdentity,
    };

    #[test]
    fn manifest_codec_rejects_wrong_magic_version_and_trailing_bytes() {
        let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
        let mut manifest = FunctionFragmentObjectContainerManifest {
            identity: FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(
                b"pending",
            ),
            stage: FunctionFragmentObjectContainerStage::ValidatedRelocationFreeObjectContainerV1,
            source_text_section_manifest:
                FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"source"),
            text_section: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"text"),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            selections: OptimizationSelectionIdentity::from_bytes([2; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([3; 32]),
            target: NativeTarget::linux_x64(),
            semantic_entry: MachineId::new(1).unwrap(),
            semantic_entry_symbol: ObjectLocalSymbolId::new(1).unwrap(),
            symbol_policy: RelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
            object: RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"object"),
            object_container: RelocationFreeObjectContainerIdentity::from_canonical_bytes(
                b"container",
            ),
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
}
