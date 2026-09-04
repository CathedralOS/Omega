//! Canonical object-container manifest encoding.

use super::*;

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
