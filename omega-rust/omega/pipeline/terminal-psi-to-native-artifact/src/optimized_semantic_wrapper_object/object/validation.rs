use crate::optimized_semantic_wrapper_object::error::*;
use crate::optimized_semantic_wrapper_object::model::*;
use crate::optimized_semantic_wrapper_object::shared::*;

pub(crate) fn validate_object(
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectError> {
    if object.recomputed_identity()? != object.identity
        || object.target != NativeTarget::uefi_x64()
        || object.target.object_format != ObjectFormat::Coff
        || object.text_section_name != section_name(object.target, SectionKind::Text)
        || object.text_section_alignment != 1
        || object.wrapper_byte_count != X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT as u64
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
    let field_start = usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET);
    let field_end = field_start + usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_WIDTH);
    let encoded_displacement = object
        .text_bytes
        .get(field_start..field_end)
        .and_then(|bytes| bytes.try_into().ok())
        .map(i32::from_le_bytes);
    if cursor != text_byte_count
        || wrapper_count != 1
        || entry_count != 1
        || object.symbols.first().map(|symbol| symbol.symbol) != Some(object.wrapper_symbol)
        || resolution.wrapper_section_offset != 0
        || resolution.next_instruction_section_offset
            != u64::from(X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET)
        || object
            .text_bytes
            .get(usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET))
            != Some(&0xe8)
        || encoded_displacement != Some(resolution.displacement)
        || i128::from(resolution.next_instruction_section_offset)
            + i128::from(resolution.displacement)
            != i128::from(resolution.continuation_section_offset)
    {
        return Err(OptimizedProgramStorageSemanticWrapperObjectError::InvalidObject);
    }
    Ok(())
}
