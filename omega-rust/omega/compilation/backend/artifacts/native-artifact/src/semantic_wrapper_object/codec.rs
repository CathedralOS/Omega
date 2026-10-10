//! Canonical wire encoding for the wrapper object plan, its container, and
//! its manifest. Decoding re-derives every identity from the wire and
//! re-validates the plan's template-free shape before returning it.

use super::manifest::valid_manifest_shape;
use super::{
    CODEC_VERSION, CONTAINER_MAGIC, MANIFEST_MAGIC,
    OptimizedProgramStorageSemanticWrapperCallResolution,
    OptimizedProgramStorageSemanticWrapperCallResolutionState,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
    OptimizedProgramStorageSemanticWrapperObjectStage,
    OptimizedProgramStorageSemanticWrapperObjectSymbol,
    OptimizedProgramStorageSemanticWrapperObjectSymbolRole,
    OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
};
use crate::cursor::{Cursor, TakeError, Truncated};
use isa_x86_64::ValidatedX86_64SemanticUnitWrapperTemplate;
use object_file::ObjectLocalSymbolId;
use optimization_core::{
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
};
use target::{NativeTarget, ObjectFormat};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

/// Encode a plan `construct_object` sealed in this compile, without re-running
/// its shape and template checks.
pub(crate) fn encode_sealed_optimized_program_storage_semantic_wrapper_object(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
> {
    encode_validated_container(object)
}

fn encode_validated_container(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(CONTAINER_MAGIC);
    crate::codec_bytes::push_encoded(&mut bytes, CODEC_VERSION.to_le_bytes());
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

pub(super) fn encode_plan_content(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<Vec<u8>, OptimizedProgramStorageSemanticWrapperObjectRecordError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&object.source_artifact.bytes());
    bytes.extend_from_slice(&object.source_artifact_manifest.bytes());
    bytes.extend_from_slice(&object.source_object.bytes());
    bytes.extend_from_slice(&object.source_object_container.bytes());
    bytes.extend_from_slice(&object.source_signature);
    terminal_codec::append_terminal_identity(&mut bytes, object.psi);
    encode_target(&mut bytes, object.target);
    encode_string(&mut bytes, &object.text_section_name)?;
    crate::codec_bytes::push_encoded(&mut bytes, object.text_section_alignment.to_le_bytes());
    encode_bytes(&mut bytes, &object.text_bytes)?;
    bytes.extend_from_slice(
        &u64::try_from(object.symbols.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?
            .to_le_bytes(),
    );
    for symbol in &object.symbols {
        crate::codec_bytes::push_encoded(&mut bytes, symbol.symbol.get().to_le_bytes());
        match symbol.source_function_index {
            Some(index) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(&mut bytes, index.to_le_bytes());
            }
            None => bytes.push(0),
        }
        match symbol.machine {
            Some(machine) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(&mut bytes, machine.get().to_le_bytes());
            }
            None => bytes.push(0),
        }
        encode_string(&mut bytes, &symbol.name)?;
        crate::codec_bytes::push_encoded(&mut bytes, symbol.section_offset.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, symbol.byte_count.to_le_bytes());
        bytes.push(match symbol.role {
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1 => 1,
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1 => 2,
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1 => 3,
        });
    }
    crate::codec_bytes::push_encoded(&mut bytes, object.wrapper_symbol.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, object.continuation_symbol.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, object.wrapper_byte_count.to_le_bytes());
    bytes.push(1);
    crate::codec_bytes::push_encoded(
        &mut bytes,
        object.call_resolution.wrapper_section_offset.to_le_bytes(),
    );
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
    crate::codec_bytes::push_encoded(
        &mut bytes,
        object.call_resolution.displacement.to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(&mut bytes, object.relocation_record_count.to_le_bytes());
    Ok(bytes)
}

pub(super) fn encode_manifest_content(
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
    terminal_codec::append_terminal_identity(bytes, manifest.psi);
    encode_target(bytes, manifest.target);
    crate::codec_bytes::push_encoded(bytes, manifest.wrapper_symbol.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, manifest.continuation_symbol.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, manifest.text_byte_count.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, manifest.symbol_count.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, manifest.relocation_record_count.to_le_bytes());
    bytes.extend_from_slice(&[1, 1, 1, 1]);
}

pub(super) fn encode_manifest(
    manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MANIFEST_MAGIC);
    crate::codec_bytes::push_encoded(&mut bytes, CODEC_VERSION.to_le_bytes());
    bytes.extend_from_slice(&manifest.identity.bytes());
    encode_manifest_content(&mut bytes, manifest);
    bytes
}

pub(super) fn decode_manifest(
    bytes: &[u8],
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
> {
    let mut cursor = Cursor::new(bytes);
    if cursor.take(8)? != MANIFEST_MAGIC {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != CODEC_VERSION {
        return Err(
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnsupportedVersion(version),
        );
    }
    let identity =
        OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_bytes(cursor.array()?);
    if cursor.byte()? != 1 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag);
    }
    let object = OptimizedProgramStorageSemanticWrapperObjectIdentity::from_bytes(cursor.array()?);
    let container =
        OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_bytes(cursor.array()?);
    let source_artifact = OptimizedObjectArtifactIdentity::from_bytes(cursor.array()?);
    let source_artifact_manifest =
        OptimizedObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
    let source_object = RelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
    let source_object_container =
        RelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
    let source_signature = cursor.array()?;
    let psi = decode_psi(&mut cursor)?;
    let target = decode_target(&mut cursor)?;
    let wrapper_symbol = decode_symbol_id(&mut cursor)?;
    let continuation_symbol = decode_symbol_id(&mut cursor)?;
    let text_byte_count = cursor.u64()?;
    let symbol_count = cursor.u64()?;
    let relocation_record_count = cursor.u64()?;
    for _ in 0..4 {
        if cursor.byte()? != 1 {
            return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag);
        }
    }
    if cursor.remaining() != 0 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::TrailingBytes);
    }
    let unavailable = OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable;
    let manifest = OptimizedProgramStorageSemanticWrapperObjectManifest {
        identity,
        stage:
            OptimizedProgramStorageSemanticWrapperObjectStage::ValidatedResolvedCompositeObjectV1,
        object,
        container,
        source_artifact,
        source_artifact_manifest,
        source_object,
        source_object_container,
        source_signature,
        psi,
        target,
        wrapper_symbol,
        continuation_symbol,
        text_byte_count,
        symbol_count,
        relocation_record_count,
        physical_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    if !valid_manifest_shape(&manifest) || manifest.recomputed_identity() != identity {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch);
    }
    Ok(manifest)
}

fn decode_psi(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
    let marker = cursor.u16()?;
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

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
    let architecture = cursor.byte()?;
    let object_format = cursor.byte()?;
    let pointer_size = cursor.u64()?;
    let pointer_alignment = cursor.u64()?;
    if architecture != 1 || object_format != 1 || pointer_size != 8 || pointer_alignment != 8 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidTarget);
    }
    Ok(NativeTarget::uefi_x64())
}

fn encode_string(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectRecordError> {
    encode_bytes(bytes, value.as_bytes())
}

fn encode_bytes(
    output: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectRecordError> {
    output.extend_from_slice(
        &u64::try_from(bytes.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
    Ok(())
}

fn decode_symbol_id(
    cursor: &mut Cursor<'_>,
) -> Result<ObjectLocalSymbolId, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
    ObjectLocalSymbolId::new(cursor.u64()?)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidSymbol)
}

impl From<Truncated> for OptimizedProgramStorageSemanticWrapperObjectDecodeError {
    fn from(_: Truncated) -> Self {
        Self::Truncated
    }
}

impl From<TakeError> for OptimizedProgramStorageSemanticWrapperObjectDecodeError {
    fn from(error: TakeError) -> Self {
        match error {
            TakeError::Truncated => Self::Truncated,
            TakeError::LengthOverflow => Self::InvalidLength,
        }
    }
}

