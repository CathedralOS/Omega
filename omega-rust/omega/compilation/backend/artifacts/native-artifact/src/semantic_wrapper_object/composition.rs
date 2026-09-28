//! Composition of the sealed wrapper object plan from one relocation-free
//! Terminal child and the validated wrapper template that is its byte
//! authority.

use super::validation::validate_object_preserving_seal;
use super::{
    OptimizedProgramStorageSemanticWrapperCallResolution,
    OptimizedProgramStorageSemanticWrapperCallResolutionState,
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
    OptimizedProgramStorageSemanticWrapperObjectSymbol,
    OptimizedProgramStorageSemanticWrapperObjectSymbolRole, WRAPPER_SYMBOL_NAME,
};
use isa_x86_64::{
    ValidatedX86_64SemanticUnitWrapperTemplate,
    resolve_x86_64_semantic_unit_wrapper_private_continuation,
};
use object_file::{ObjectLocalSymbolId, RelocationFreeObjectPlan, RelocationFreeObjectSymbolRole};
use optimization_core::{
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity, RelocationFreeObjectContainerIdentity,
};

/// Builds and seals the composite plan: the resolved wrapper occupies the
/// first section bytes and every child symbol shifts behind it. The caller
/// owns the joins that bind `child` and the source identities to their
/// settlement; this constructor binds only the object-format facts.
pub fn compose_optimized_program_storage_semantic_wrapper_object(
    source_signature: [u8; 32],
    source_artifact: OptimizedObjectArtifactIdentity,
    source_artifact_manifest: OptimizedObjectArtifactManifestIdentity,
    source_object_container: RelocationFreeObjectContainerIdentity,
    child: &RelocationFreeObjectPlan,
    template: &ValidatedX86_64SemanticUnitWrapperTemplate,
) -> Result<
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
> {
    // The wrapper object format has no relocation records: a child still
    // carrying unresolved foreign import fields cannot be composed into it.
    if child.relocation_record_count != 0 {
        return Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::SourceObjectMismatch);
    }
    // The encoding's validated template is the wrapper's byte authority: a
    // provisioned receiver widens the canonical receiver-free geometry.
    let wrapper_byte_count = u64::try_from(template.bytes().len())
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?;
    let child_entry = child
        .symbols
        .iter()
        .find(|symbol| symbol.symbol == child.semantic_entry_symbol)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectRecordError::SourceObjectMismatch)?;
    let continuation_section_offset = wrapper_byte_count
        .checked_add(child_entry.section_offset)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?;
    let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
        template,
        template.relocation(),
        0,
        continuation_section_offset,
    )
    .map_err(OptimizedProgramStorageSemanticWrapperObjectRecordError::WrapperResolution)?;
    let mut text_bytes = Vec::with_capacity(
        resolved
            .bytes()
            .len()
            .checked_add(child.text_section.bytes.len())
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?,
    );
    text_bytes.extend_from_slice(resolved.bytes());
    text_bytes.extend_from_slice(&child.text_section.bytes);
    let wrapper_symbol = ObjectLocalSymbolId::new(1)
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?;
    let mut symbols = Vec::with_capacity(
        child
            .symbols
            .len()
            .checked_add(1)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?,
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
                .map_err(|_| {
                    OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow
                })?
                .checked_add(2)
                .ok_or(OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?,
        )
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?;
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
                .ok_or(OptimizedProgramStorageSemanticWrapperObjectRecordError::LengthOverflow)?,
            byte_count: symbol.byte_count,
            role,
        });
    }
    let continuation_symbol = continuation_symbol
        .ok_or(OptimizedProgramStorageSemanticWrapperObjectRecordError::SourceObjectMismatch)?;
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
    validate_object_preserving_seal(&object, template)?;
    Ok(object)
}
