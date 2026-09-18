//! Structural offsets for the canonical ProgramStorage wrapper object envelope.
//!
//! The walk mirrors `encode_plan_content` byte-for-byte and asserts every
//! length, tag, and role it crosses so a drifted layout fails loudly instead
//! of corrupting a neighboring axis.

use super::super::OptimizedProgramStorageSemanticWrapperObjectPlan;

#[derive(Debug, Clone, Copy)]
pub(super) struct SymbolRowOffsets {
    pub symbol: usize,
    pub source_function_index_tag: usize,
    pub machine_tag: usize,
    pub machine: Option<usize>,
    pub name: usize,
    pub role: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WireOffsets {
    pub identity: usize,
    pub vocabulary: usize,
    pub architecture: usize,
    pub object_format: usize,
    pub pointer_size: usize,
    pub pointer_alignment: usize,
    pub text_section_name_len: usize,
    pub text_bytes: usize,
    pub symbol_rows: [SymbolRowOffsets; 2],
    pub wrapper_symbol: usize,
    pub continuation_symbol: usize,
    pub call_state: usize,
    pub displacement: usize,
}

fn take_u64(bytes: &[u8], cursor: &mut usize) -> usize {
    let value = usize::try_from(u64::from_le_bytes(
        bytes[*cursor..*cursor + 8].try_into().unwrap(),
    ))
    .unwrap();
    *cursor += 8;
    value
}

fn symbol_row(bytes: &[u8], cursor: &mut usize) -> SymbolRowOffsets {
    let symbol = *cursor;
    *cursor += 8;
    let source_function_index_tag = *cursor;
    *cursor += 1;
    if bytes[source_function_index_tag] == 1 {
        *cursor += 8;
    }
    let machine_tag = *cursor;
    *cursor += 1;
    let machine = (bytes[machine_tag] == 1).then(|| {
        let offset = *cursor;
        *cursor += 8;
        offset
    });
    let name_len = take_u64(bytes, cursor);
    let name = *cursor;
    *cursor += name_len;
    *cursor += 16; // section_offset + byte_count
    let role = *cursor;
    *cursor += 1;
    SymbolRowOffsets {
        symbol,
        source_function_index_tag,
        machine_tag,
        machine,
        name,
        role,
    }
}

pub(super) fn wire_offsets(
    bytes: &[u8],
    object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> WireOffsets {
    assert_eq!(
        object.symbols.len(),
        2,
        "fixture must carry two symbol rows"
    );
    let identity = 12; // magic(8) + version(4)
    let mut cursor = identity + 32;
    cursor += 5 * 32; // upstream identities + source_signature
    let vocabulary = cursor;
    cursor += 2 + 32; // vocabulary marker + program fingerprint
    let architecture = cursor;
    let object_format = cursor + 1;
    let pointer_size = cursor + 2;
    let pointer_alignment = cursor + 10;
    cursor += 18;
    assert_eq!(&bytes[vocabulary..vocabulary + 2], &107_u16.to_le_bytes());
    assert_eq!(bytes[architecture], 1);
    assert_eq!(bytes[object_format], 1);

    let text_section_name_len = cursor;
    let name_len = take_u64(bytes, &mut cursor);
    assert_eq!(name_len, object.text_section_name.len());
    cursor += name_len;
    cursor += 8; // text_section_alignment

    let text_len = take_u64(bytes, &mut cursor);
    assert_eq!(text_len, object.text_bytes.len());
    let text_bytes = cursor;
    cursor += text_len;

    let symbol_count = take_u64(bytes, &mut cursor);
    assert_eq!(symbol_count, object.symbols.len());
    let wrapper_row = symbol_row(bytes, &mut cursor);
    assert_eq!(bytes[wrapper_row.role], 1);
    assert_eq!(bytes[wrapper_row.source_function_index_tag], 0);
    assert_eq!(bytes[wrapper_row.machine_tag], 0);
    let continuation_row = symbol_row(bytes, &mut cursor);
    assert_eq!(bytes[continuation_row.role], 2);
    assert_eq!(bytes[continuation_row.source_function_index_tag], 1);
    assert_eq!(bytes[continuation_row.machine_tag], 1);

    let wrapper_symbol = cursor;
    cursor += 8;
    let continuation_symbol = cursor;
    cursor += 8;
    cursor += 8; // wrapper_byte_count
    let call_state = cursor;
    assert_eq!(bytes[call_state], 1);
    cursor += 1;
    cursor += 8 + 8 + 8; // wrapper/continuation/next-instruction section offsets
    let displacement = cursor;
    cursor += 4;
    cursor += 8; // relocation_record_count
    assert_eq!(cursor, bytes.len());

    WireOffsets {
        identity,
        vocabulary,
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
        text_section_name_len,
        text_bytes,
        symbol_rows: [wrapper_row, continuation_row],
        wrapper_symbol,
        continuation_symbol,
        call_state,
        displacement,
    }
}
