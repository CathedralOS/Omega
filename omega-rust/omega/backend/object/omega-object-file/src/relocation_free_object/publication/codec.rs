//! Canonical version-1 publication encoding, not admission against a source.
use super::*;
const MANIFEST_MAGIC: &[u8; 8] = b"OMGTOM\0\0";
const MANIFEST_VERSION: u32 = 1;
impl FunctionFragmentObjectContainerManifest {
    pub fn recomputed_identity(&self) -> FunctionFragmentObjectContainerManifestIdentity {
        let mut canonical = b"omega.function-fragment-object-container-manifest.v1\0".to_vec();
        canonical.extend_from_slice(&encode_manifest_content(self));
        FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44_usize.saturating_add(content.len()));
        encoded.extend_from_slice(MANIFEST_MAGIC);
        encoded.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, FunctionFragmentObjectContainerManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
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
        let marker = u16::from_le_bytes(cursor.array()?);
        let vocabulary_marker = VocabularyMarker::new(marker)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::UnknownVocabulary(marker))?;
        let psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        };
        let fuel = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(fuel)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidFuelSchedule)?;
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)?;
        let semantic_entry = MachineId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidSemanticEntry)?;
        let semantic_entry_symbol =
            ObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
                .ok_or(FunctionFragmentObjectContainerManifestDecodeError::InvalidSymbolId)?;
        if cursor.byte()? != 1 {
            return Err(FunctionFragmentObjectContainerManifestDecodeError::UnknownSymbolPolicy);
        }
        let object = RelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
        let object_container = RelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
        if cursor.byte()? != 1 {
            return Err(
                FunctionFragmentObjectContainerManifestDecodeError::UnknownRelocationRequirements,
            );
        }
        let statistics = FunctionFragmentObjectContainerStatistics {
            sections: u64::from_le_bytes(cursor.array()?),
            function_symbols: u64::from_le_bytes(cursor.array()?),
            object_local_symbols: u64::from_le_bytes(cursor.array()?),
            external_symbols: u64::from_le_bytes(cursor.array()?),
            text_bytes: u64::from_le_bytes(cursor.array()?),
            container_bytes: u64::from_le_bytes(cursor.array()?),
            relocation_records: u64::from_le_bytes(cursor.array()?),
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
            symbol_policy:
                RelocationFreeObjectSymbolPolicy::PrivateSemanticMachineSymbolsV1,
            object,
            object_container,
            relocation_requirements:
                RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
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
    bytes.extend_from_slice(&record.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(record.psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&record.fuel_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&record.selections.bytes());
    bytes.extend_from_slice(&record.selected.bytes());
    encode_target(&mut bytes, record.target);
    bytes.extend_from_slice(&record.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&record.semantic_entry_symbol.get().to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&record.object.bytes());
    bytes.extend_from_slice(&record.object_container.bytes());
    bytes.push(1);
    bytes.extend_from_slice(&record.statistics.sections.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.function_symbols.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.object_local_symbols.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.external_symbols.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.text_bytes.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.container_bytes.to_le_bytes());
    bytes.extend_from_slice(&record.statistics.relocation_records.to_le_bytes());
    bytes.extend_from_slice(&[1; 4]);
    bytes
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
) -> Result<NativeTarget, FunctionFragmentObjectContainerManifestDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::Aarch64,
        2 => Architecture::X86_64,
        tag => {
            return Err(
                FunctionFragmentObjectContainerManifestDecodeError::UnknownArchitecture(tag),
            );
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(
                FunctionFragmentObjectContainerManifestDecodeError::UnknownObjectFormat(tag),
            );
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentObjectContainerManifestDecodeError::TargetLayoutOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| FunctionFragmentObjectContainerManifestDecodeError::TargetLayoutOverflow)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
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
    ) -> Result<&'a [u8], FunctionFragmentObjectContainerManifestDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(FunctionFragmentObjectContainerManifestDecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    pub(super) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], FunctionFragmentObjectContainerManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FunctionFragmentObjectContainerManifestDecodeError::Truncated)
    }

    pub(super) fn byte(
        &mut self,
    ) -> Result<u8, FunctionFragmentObjectContainerManifestDecodeError> {
        Ok(self.array::<1>()?[0])
    }
}
