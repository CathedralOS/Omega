//! Translate a checked source primitive into the closed realization vocabulary.

pub const fn primitive_numeric_type(
    primitive: typed_trees::types::PrimitiveType,
) -> Option<effects::CompilerNumericType> {
    use typed_trees::types::PrimitiveType;

    match primitive {
        PrimitiveType::I8 => Some(effects::CompilerNumericType::I8),
        PrimitiveType::I16 => Some(effects::CompilerNumericType::I16),
        PrimitiveType::I32 => Some(effects::CompilerNumericType::I32),
        PrimitiveType::I64 => Some(effects::CompilerNumericType::I64),
        PrimitiveType::U8 => Some(effects::CompilerNumericType::U8),
        PrimitiveType::U16 => Some(effects::CompilerNumericType::U16),
        PrimitiveType::U32 => Some(effects::CompilerNumericType::U32),
        PrimitiveType::U64 => Some(effects::CompilerNumericType::U64),
        PrimitiveType::F32 => Some(effects::CompilerNumericType::F32),
        PrimitiveType::F64 => Some(effects::CompilerNumericType::F64),
        PrimitiveType::Bool | PrimitiveType::Addr | PrimitiveType::Selector => None,
    }
}
