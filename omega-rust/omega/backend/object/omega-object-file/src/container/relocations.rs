use super::bytes::{write_i64, write_string, write_u32, write_u64};
use super::ids::{relocation_kind_id, section_kind_id};
use crate::{ObjectPlan, ObjectSymbolHandle, RelocationOrigin, RelocationPlan, object_symbol_name};

fn encoded_origin(origin: RelocationOrigin) -> (u32, ObjectSymbolHandle, u64) {
    match origin {
        RelocationOrigin::Instruction {
            function_symbol_handle,
            selected_instruction_index,
        } => (
            1,
            function_symbol_handle,
            u64::from(selected_instruction_index),
        ),
        RelocationOrigin::Materialization {
            object_symbol_handle,
        } => (2, object_symbol_handle, 0),
        RelocationOrigin::SemanticOperation {
            function_symbol_handle,
            operation_identity,
        } => (3, function_symbol_handle, operation_identity),
        RelocationOrigin::SemanticEdge {
            function_symbol_handle,
            edge_identity,
        } => (4, function_symbol_handle, edge_identity),
    }
}

pub(super) fn write_relocations(
    bytes: &mut Vec<u8>,
    object: &ObjectPlan,
    relocations: &RelocationPlan,
) {
    write_u32(
        bytes,
        u32::try_from(relocations.record_count()).expect("relocation count overflow"),
    );

    for (_, relocation) in relocations.records() {
        let (origin_id, origin_symbol, origin_identity) = encoded_origin(relocation.origin);
        write_u32(bytes, origin_id);
        write_string(bytes, object_symbol_name(object, origin_symbol));
        write_u64(bytes, origin_identity);
        write_u32(bytes, section_kind_id(relocation.section));
        write_u64(
            bytes,
            u64::try_from(relocation.offset).expect("relocation section offset overflow"),
        );
        write_u32(
            bytes,
            u32::try_from(relocation.byte_width).expect("relocation byte width overflow"),
        );
        write_string(bytes, object_symbol_name(object, relocation.symbol_handle));
        write_i64(bytes, relocation.addend);
        write_u32(bytes, relocation_kind_id(relocation.kind));
    }
}

#[cfg(test)]
mod tests {
    use super::encoded_origin;
    use crate::RelocationOrigin;
    use psi_arena::Handle;

    #[test]
    fn semantic_edge_has_a_distinct_container_tag_from_semantic_operation() {
        let function_symbol_handle = Handle::invalid();
        assert_eq!(
            encoded_origin(RelocationOrigin::SemanticOperation {
                function_symbol_handle,
                operation_identity: 11,
            }),
            (3, function_symbol_handle, 11)
        );
        assert_eq!(
            encoded_origin(RelocationOrigin::SemanticEdge {
                function_symbol_handle,
                edge_identity: 11,
            }),
            (4, function_symbol_handle, 11)
        );
    }
}
