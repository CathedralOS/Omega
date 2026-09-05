use crate::optimized_semantic_wrapper_object::error::*;
use crate::optimized_semantic_wrapper_object::model::*;
use crate::optimized_semantic_wrapper_object::object::validate_object;
use crate::optimized_semantic_wrapper_object::shared::*;

pub fn encode_optimized_program_storage_semantic_wrapper_object(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    validate_object(object)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(CONTAINER_MAGIC);
    bytes.extend_from_slice(&CODEC_VERSION.to_le_bytes());
    bytes.extend_from_slice(&object.identity.bytes());
    bytes.extend_from_slice(&encode_plan_content(object)?);
    Ok(OptimizedProgramStorageSemanticWrapperObjectContainer {
        identity:
            OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_canonical_bytes(
                &bytes,
            ),
        object: object.identity,
        bytes,
    })
}

pub fn decode_optimized_program_storage_semantic_wrapper_object(
    bytes: &[u8],
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
> {
    let mut cursor = Cursor::new(bytes);
    if cursor.take(8)? != CONTAINER_MAGIC {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::WrongMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != CODEC_VERSION {
        return Err(
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnsupportedVersion(version),
        );
    }
    let identity =
        OptimizedProgramStorageSemanticWrapperObjectIdentity::from_bytes(cursor.array()?);
    let object = decode_plan_content(&mut cursor, identity)?;
    if cursor.remaining() != 0 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::TrailingBytes);
    }
    validate_object(&object)
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidObject)?;
    Ok(object)
}

pub(crate) fn encode_plan_content(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<Vec<u8>, OptimizedProgramStorageSemanticWrapperObjectError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&object.source_artifact.bytes());
    bytes.extend_from_slice(&object.source_artifact_manifest.bytes());
    bytes.extend_from_slice(&object.source_object.bytes());
    bytes.extend_from_slice(&object.source_object_container.bytes());
    bytes.extend_from_slice(&object.source_signature);
    encode_psi(&mut bytes, object.psi);
    encode_target(&mut bytes, object.target);
    encode_string(&mut bytes, &object.text_section_name)?;
    bytes.extend_from_slice(&object.text_section_alignment.to_le_bytes());
    encode_bytes(&mut bytes, &object.text_bytes)?;
    bytes.extend_from_slice(
        &u64::try_from(object.symbols.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            .to_le_bytes(),
    );
    for symbol in &object.symbols {
        bytes.extend_from_slice(&symbol.symbol.get().to_le_bytes());
        match symbol.source_function_index {
            Some(index) => {
                bytes.push(1);
                bytes.extend_from_slice(&index.to_le_bytes());
            }
            None => bytes.push(0),
        }
        match symbol.machine {
            Some(machine) => {
                bytes.push(1);
                bytes.extend_from_slice(&machine.get().to_le_bytes());
            }
            None => bytes.push(0),
        }
        encode_string(&mut bytes, &symbol.name)?;
        bytes.extend_from_slice(&symbol.section_offset.to_le_bytes());
        bytes.extend_from_slice(&symbol.byte_count.to_le_bytes());
        bytes.push(match symbol.role {
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1 => 1,
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1 => 2,
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1 => 3,
        });
    }
    bytes.extend_from_slice(&object.wrapper_symbol.get().to_le_bytes());
    bytes.extend_from_slice(&object.continuation_symbol.get().to_le_bytes());
    bytes.extend_from_slice(&object.wrapper_byte_count.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&object.call_resolution.wrapper_section_offset.to_le_bytes());
    bytes.extend_from_slice(
        &object
            .call_resolution
            .continuation_section_offset
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &object
            .call_resolution
            .next_instruction_section_offset
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&object.call_resolution.displacement.to_le_bytes());
    bytes.extend_from_slice(&object.relocation_record_count.to_le_bytes());
    Ok(bytes)
}

fn decode_plan_content(
    cursor: &mut Cursor<'_>,
    identity: OptimizedProgramStorageSemanticWrapperObjectIdentity,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
> {
    let source_artifact = OptimizedObjectArtifactIdentity::from_bytes(cursor.array()?);
    let source_artifact_manifest =
        OptimizedObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
    let source_object = RelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
    let source_object_container =
        RelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
    let source_signature = cursor.array()?;
    let psi = decode_psi(cursor)?;
    let target = decode_target(cursor)?;
    let text_section_name = cursor.string()?;
    let text_section_alignment = u64::from_le_bytes(cursor.array()?);
    let text_bytes = cursor.bytes()?;
    let symbol_count = cursor.len()?;
    let mut symbols = Vec::with_capacity(symbol_count);
    for _ in 0..symbol_count {
        let symbol = decode_symbol_id(cursor)?;
        let source_function_index = match cursor.byte()? {
            0 => None,
            1 => Some(u64::from_le_bytes(cursor.array()?)),
            _ => return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag),
        };
        let machine = match cursor.byte()? {
            0 => None,
            1 => {
                Some(MachineId::new(u64::from_le_bytes(cursor.array()?)).ok_or(
                    OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidMachine,
                )?)
            }
            _ => return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag),
        };
        let name = cursor.string()?;
        let section_offset = u64::from_le_bytes(cursor.array()?);
        let byte_count = u64::from_le_bytes(cursor.array()?);
        let role = match cursor.byte()? {
            1 => OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1,
            2 => OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1,
            3 => OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1,
            _ => return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag),
        };
        symbols.push(OptimizedProgramStorageSemanticWrapperObjectSymbol {
            symbol,
            source_function_index,
            machine,
            name,
            section_offset,
            byte_count,
            role,
        });
    }
    let wrapper_symbol = decode_symbol_id(cursor)?;
    let continuation_symbol = decode_symbol_id(cursor)?;
    let wrapper_byte_count = u64::from_le_bytes(cursor.array()?);
    if cursor.byte()? != 1 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag);
    }
    let call_resolution = OptimizedProgramStorageSemanticWrapperCallResolution {
        state: OptimizedProgramStorageSemanticWrapperCallResolutionState::ResolvedInCompositeTextSectionV1,
        wrapper_section_offset: u64::from_le_bytes(cursor.array()?),
        continuation_section_offset: u64::from_le_bytes(cursor.array()?),
        next_instruction_section_offset: u64::from_le_bytes(cursor.array()?),
        displacement: i32::from_le_bytes(cursor.array()?),
    };
    let relocation_record_count = u64::from_le_bytes(cursor.array()?);
    let object = OptimizedProgramStorageSemanticWrapperObjectPlan {
        identity,
        source_artifact,
        source_artifact_manifest,
        source_object,
        source_object_container,
        source_signature,
        psi,
        target,
        text_section_name,
        text_section_alignment,
        text_bytes,
        symbols,
        wrapper_symbol,
        continuation_symbol,
        wrapper_byte_count,
        call_resolution,
        relocation_record_count,
    };
    if object
        .recomputed_identity()
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidObject)?
        != identity
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch);
    }
    Ok(object)
}

pub(crate) fn encode_manifest_content(
    bytes: &mut Vec<u8>,
    manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest,
) {
    bytes.push(1);
    bytes.extend_from_slice(&manifest.object.bytes());
    bytes.extend_from_slice(&manifest.container.bytes());
    bytes.extend_from_slice(&manifest.source_artifact.bytes());
    bytes.extend_from_slice(&manifest.source_artifact_manifest.bytes());
    bytes.extend_from_slice(&manifest.source_object.bytes());
    bytes.extend_from_slice(&manifest.source_object_container.bytes());
    bytes.extend_from_slice(&manifest.source_signature);
    encode_psi(bytes, manifest.psi);
    encode_target(bytes, manifest.target);
    bytes.extend_from_slice(&manifest.wrapper_symbol.get().to_le_bytes());
    bytes.extend_from_slice(&manifest.continuation_symbol.get().to_le_bytes());
    bytes.extend_from_slice(&manifest.text_byte_count.to_le_bytes());
    bytes.extend_from_slice(&manifest.symbol_count.to_le_bytes());
    bytes.extend_from_slice(&manifest.relocation_record_count.to_le_bytes());
    bytes.extend_from_slice(&[1, 1, 1, 1]);
}

fn encode_psi(bytes: &mut Vec<u8>, identity: TerminalPsiIdentity) {
    bytes.extend_from_slice(&identity.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(identity.program_fingerprint.as_bytes());
}

pub(crate) fn decode_psi(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
    let marker = u16::from_le_bytes(cursor.array()?);
    Ok(TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::new(marker)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidVocabulary)?,
        program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
    })
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(1);
    bytes.push(match target.object_format {
        ObjectFormat::Coff => 1,
        ObjectFormat::Elf => 2,
        ObjectFormat::MachO => 3,
    });
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_size)
            .unwrap_or_default()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_alignment)
            .unwrap_or_default()
            .to_le_bytes(),
    );
}

pub(crate) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
    let architecture = cursor.byte()?;
    let object_format = cursor.byte()?;
    let pointer_size = u64::from_le_bytes(cursor.array()?);
    let pointer_alignment = u64::from_le_bytes(cursor.array()?);
    if architecture != 1 || object_format != 1 || pointer_size != 8 || pointer_alignment != 8 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidTarget);
    }
    Ok(NativeTarget::uefi_x64())
}

fn encode_string(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    bytes.extend_from_slice(
        &u64::try_from(value.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn encode_bytes(
    output: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    output.extend_from_slice(
        &u64::try_from(bytes.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
    Ok(())
}

pub(crate) fn decode_symbol_id(
    cursor: &mut Cursor<'_>,
) -> Result<ObjectLocalSymbolId, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
    ObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidSymbol)
}

pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    pub(crate) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated)
    }

    pub(crate) fn byte(
        &mut self,
    ) -> Result<u8, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn len(
        &mut self,
    ) -> Result<usize, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidLength)
    }

    pub(crate) fn bytes(
        &mut self,
    ) -> Result<Vec<u8>, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        let count = self.len()?;
        Ok(self.take(count)?.to_vec())
    }

    pub(crate) fn string(
        &mut self,
    ) -> Result<String, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        String::from_utf8(self.bytes()?)
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidUtf8)
    }

    pub(crate) const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}
