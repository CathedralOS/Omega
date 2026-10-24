//! Canonical version-1 publication encoding, not admission against a source.
use super::{
    FuelScheduleIdentity, FunctionFragmentObjectContainerManifest,
    FunctionFragmentObjectContainerManifestDecodeError,
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentObjectContainerStage,
    FunctionFragmentObjectContainerStatistics, FunctionFragmentObjectContainerUnavailableData,
    FunctionFragmentTextSectionManifestIdentity, MachineId, NativeTarget, ObjectLocalSymbolId,
    OptimizationSelectionIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity, RelocationFreeObjectRelocationRequirements,
    RelocationFreeObjectSymbolPolicy, SelectedInstructionPlanIdentity, SemanticFingerprint,
    TerminalPsiIdentity, TerminalRelocationFreeTextSectionIdentity, VocabularyMarker,
};
use crate::cursor::Cursor;
const MANIFEST_MAGIC: &[u8; 8] = b"OMGTOM\0\0";
const MANIFEST_VERSION: u32 = 1;
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
        let fuel = cursor.u32()?;
        let fuel_schedule = FuelScheduleIdentity::new(fuel)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidFuelSchedule)?;
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
            fuel_schedule,
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

pub(super) fn encode_manifest_content(record: &FunctionFragmentObjectContainerManifest) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(1);
    bytes.extend_from_slice(&record.source_text_section_manifest.bytes());
    bytes.extend_from_slice(&record.text_section.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(record.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, record.fuel_schedule.marker().to_le_bytes());
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
