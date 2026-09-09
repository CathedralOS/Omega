//! Byte payloads and checked view coordinates retain exact scalar carriers.
use super::*;

pub(super) fn types_match(operation: &O, definitions: &BTreeMap<ValueId, ValueDefinition>) -> bool {
    let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
    match operation {
        O::ByteSequenceWrite {
            index,
            value,
            length,
            ..
        } => {
            matches!(scalar(*value), Some(ScalarType::Integer(integer))
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 8))
                && matches!(scalar(*index), Some(ScalarType::Integer(integer))
                    if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
                && scalar(*index) == scalar(*length)
        }
        O::ByteSequenceSubslice {
            start, end, length, ..
        } => {
            matches!(scalar(*start), Some(ScalarType::Integer(integer))
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
                && scalar(*start) == scalar(*end)
                && scalar(*start) == scalar(*length)
        }
        O::ByteSequenceRead {
            result,
            index,
            length,
            ..
        } => {
            matches!(result.scalar_type, ScalarType::Integer(integer)
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 8))
                && matches!(scalar(*index), Some(ScalarType::Integer(integer))
                    if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
                && scalar(*index) == scalar(*length)
        }
        O::ByteSequenceLength { result, .. } => {
            matches!(result.scalar_type, ScalarType::Integer(integer)
                if Ok(integer) == IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64))
        }
        _ => false,
    }
}
