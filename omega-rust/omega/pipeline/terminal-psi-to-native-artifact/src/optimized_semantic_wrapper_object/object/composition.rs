use crate::optimized_semantic_wrapper_object::error::*;
use crate::optimized_semantic_wrapper_object::model::*;
use crate::optimized_semantic_wrapper_object::shared::*;

use super::validate_object;

pub(crate) fn compose_object(
    source_signature: [u8; 32],
    source_artifact: OptimizedObjectArtifactIdentity,
    source_artifact_manifest: OptimizedObjectArtifactManifestIdentity,
    source_object_container: RelocationFreeObjectContainerIdentity,
    child: &RelocationFreeObjectPlan,
    encoding: &StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectError,
> {
    let wrapper_byte_count = u64::try_from(encoding.template().bytes().len())
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
    if wrapper_byte_count != X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT as u64 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    let child_entry = child
        .symbols
        .iter()
        .find(|symbol| symbol.symbol == child.semantic_entry_symbol)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::SourceObjectMismatch)?;
    let continuation_section_offset = wrapper_byte_count
        .checked_add(child_entry.section_offset)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
    let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
        encoding.template(),
        encoding.template().relocation(),
        0,
        continuation_section_offset,
    )
    .map_err(OptimizedProgramStorageSemanticWrapperObjectError::WrapperResolution)?;
    let mut text_bytes = Vec::with_capacity(
        resolved
            .bytes()
            .len()
            .checked_add(child.text_section.bytes.len())
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
    );
    text_bytes.extend_from_slice(resolved.bytes());
    text_bytes.extend_from_slice(&child.text_section.bytes);
    let wrapper_symbol = ObjectLocalSymbolId::new(1)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
    let mut symbols = Vec::with_capacity(
        child
            .symbols
            .len()
            .checked_add(1)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
    );
    symbols.push(OptimizedProgramStorageSemanticWrapperObjectSymbol {
        symbol: wrapper_symbol,
        source_function_index: None,
        machine: None,
        name: WRAPPER_SYMBOL_NAME.into(),
        section_offset: 0,
        byte_count: wrapper_byte_count,
        role: OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1,
    });
    let mut continuation_symbol = None;
    for (index, symbol) in child.symbols.iter().enumerate() {
        let new_symbol = ObjectLocalSymbolId::new(
            u64::try_from(index)
                .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
                .checked_add(2)
                .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
        )
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
        let role = match symbol.role {
            RelocationFreeObjectSymbolRole::SemanticEntryV1 => {
                continuation_symbol = Some(new_symbol);
                OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1
            }
            RelocationFreeObjectSymbolRole::PrivateFunctionV1 => {
                OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1
            }
        };
        symbols.push(OptimizedProgramStorageSemanticWrapperObjectSymbol {
            symbol: new_symbol,
            source_function_index: Some(symbol.source_function_index),
            machine: Some(symbol.machine),
            name: symbol.name.clone(),
            section_offset: wrapper_byte_count
                .checked_add(symbol.section_offset)
                .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?,
            byte_count: symbol.byte_count,
            role,
        });
    }
    let continuation_symbol = continuation_symbol
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::SourceObjectMismatch)?;
    let resolution = resolved.resolution();
    let mut object = OptimizedProgramStorageSemanticWrapperObjectPlan {
        identity: OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(
            b"pending",
        ),
        source_artifact,
        source_artifact_manifest,
        source_object: child.identity,
        source_object_container,
        source_signature,
        psi: child.psi,
        target: child.target,
        text_section_name: child.text_section.name.clone(),
        text_section_alignment: child.text_section.alignment,
        text_bytes,
        symbols,
        wrapper_symbol,
        continuation_symbol,
        wrapper_byte_count,
        call_resolution: OptimizedProgramStorageSemanticWrapperCallResolution {
            state: OptimizedProgramStorageSemanticWrapperCallResolutionState::ResolvedInCompositeTextSectionV1,
            wrapper_section_offset: resolution.wrapper_section_offset,
            continuation_section_offset: resolution.continuation_section_offset,
            next_instruction_section_offset: resolution.next_instruction_section_offset,
            displacement: resolution.displacement,
        },
        relocation_record_count: 0,
    };
    object.identity = object.recomputed_identity()?;
    validate_object(&object)?;
    Ok(object)
}
