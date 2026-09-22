use crate::optimized_semantic_wrapper_object::error::OptimizedProgramStorageSemanticWrapperObjectError;
use crate::optimized_semantic_wrapper_object::model::WRAPPER_SYMBOL_NAME;
use crate::optimized_semantic_wrapper_object::model::{
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectSymbolRole,
};
use isa_x86_64::ValidatedX86_64SemanticUnitWrapperTemplate;
use object_file::{SectionKind, canonical_private_machine_symbol_name, section_name};
use std::collections::BTreeSet;
use target::{NativeTarget, ObjectFormat};

/// The template-free object invariants: symbol ordering, section custody,
/// and the resolved-call equation. Byte coordinates inside the wrapper are
/// authority of the validated encoding template — `validate_object` adds
/// those cross-checks; the codec's standalone path can only assert shape.
pub(crate) fn validate_object_shape(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    if object.recomputed_identity()? != object.identity {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    validate_object_shape_content(object)
}

/// Every `validate_object_shape` conjunct below the identity seal. Callers
/// that assigned or verified `object.identity` against `recomputed_identity()`
/// on the same unchanged in-memory value know the digest conjunct cannot
/// differ, so they validate through this instead of re-deriving the seal.
pub(crate) fn validate_object_shape_content(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    if object.target != NativeTarget::uefi_x64()
        || object.target.object_format != ObjectFormat::Coff
        || object.text_section_name != section_name(object.target, SectionKind::Text)
        || object.text_section_alignment != 1
        || object.relocation_record_count != 0
        || u64::try_from(object.text_bytes.len())
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            < object.wrapper_byte_count
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    let mut names = BTreeSet::new();
    let mut machines = BTreeSet::new();
    let mut cursor = 0_u64;
    let mut wrapper_count = 0;
    let mut entry_count = 0;
    for (index, symbol) in object.symbols.iter().enumerate() {
        let expected_id = u64::try_from(index)
            .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?
            .checked_add(1)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
        if symbol.symbol.get() != expected_id
            || symbol.section_offset != cursor
            || !names.insert(symbol.name.as_str())
        {
            return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
        }
        cursor = cursor
            .checked_add(symbol.byte_count)
            .ok_or(OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
        match symbol.role {
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1 => {
                wrapper_count += 1;
                if symbol.symbol != object.wrapper_symbol
                    || symbol.source_function_index.is_some()
                    || symbol.machine.is_some()
                    || symbol.name != WRAPPER_SYMBOL_NAME
                    || symbol.section_offset != 0
                    || symbol.byte_count != object.wrapper_byte_count
                {
                    return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
                }
            }
            OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1
            | OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalFunctionV1 => {
                let (Some(source_index), Some(machine)) =
                    (symbol.source_function_index, symbol.machine)
                else {
                    return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
                };
                if source_index
                    != u64::try_from(
                        index.checked_sub(1).ok_or(
                            OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject,
                        )?,
                    )
                    .map_err(|_| {
                        OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow
                    })?
                    || !machines.insert(machine)
                    || symbol.name != canonical_private_machine_symbol_name(machine)
                {
                    return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
                }
                if symbol.role
                    == OptimizedProgramStorageSemanticWrapperObjectSymbolRole::PrivateTerminalContinuationV1
                {
                    entry_count += 1;
                    if symbol.symbol != object.continuation_symbol
                        || symbol.section_offset
                            != object.call_resolution.continuation_section_offset
                    {
                        return Err(
                            OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject,
                        );
                    }
                }
            }
        }
    }
    let text_byte_count = u64::try_from(object.text_bytes.len())
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
    let resolution = object.call_resolution;
    if cursor != text_byte_count
        || wrapper_count != 1
        || entry_count != 1
        || object.symbols.first().map(|symbol| symbol.symbol) != Some(object.wrapper_symbol)
        || resolution.wrapper_section_offset != 0
        || i128::from(resolution.next_instruction_section_offset)
            + i128::from(resolution.displacement)
            != i128::from(resolution.continuation_section_offset)
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    Ok(())
}

/// The join's full object check: the validated encoding template supplies the
/// canonical byte geometry — receiver variants shift every call coordinate —
/// so the composite must agree with it exactly.
pub(crate) fn validate_object(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
    wrapper: &ValidatedX86_64SemanticUnitWrapperTemplate,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    validate_object_shape(object)?;
    validate_object_against_template(object, wrapper)
}

/// The join check for an object the caller just sealed: `object.identity` was
/// assigned from `recomputed_identity()` at construction, so the digest
/// conjunct cannot differ; every other shape and template check still runs.
pub(crate) fn validate_object_preserving_seal(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
    wrapper: &ValidatedX86_64SemanticUnitWrapperTemplate,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    validate_object_shape_content(object)?;
    validate_object_against_template(object, wrapper)
}

fn validate_object_against_template(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
    wrapper: &ValidatedX86_64SemanticUnitWrapperTemplate,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    let wrapper_byte_count = u64::try_from(wrapper.bytes().len())
        .map_err(|_| OptimizedProgramStorageSemanticWrapperObjectError::LengthOverflow)?;
    let relocation = wrapper.relocation();
    let field_start = usize::from(relocation.field_function_byte_offset);
    let field_end = field_start + usize::from(relocation.field_byte_width);
    let encoded_displacement = object
        .text_bytes
        .get(field_start..field_end)
        .and_then(|bytes| bytes.try_into().ok())
        .map(i32::from_le_bytes);
    if object.wrapper_byte_count != wrapper_byte_count
        || object.call_resolution.next_instruction_section_offset
            != u64::from(relocation.next_instruction_function_byte_offset)
        || object
            .text_bytes
            .get(usize::from(relocation.opcode_function_byte_offset))
            != Some(&0xe8)
        || encoded_displacement != Some(object.call_resolution.displacement)
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    Ok(())
}
