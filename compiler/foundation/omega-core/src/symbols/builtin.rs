use super::{SymbolKind, SymbolNameRef};

pub fn builtin_type_symbols() -> [(SymbolKind, SymbolNameRef<'static>); 19] {
    [
        (SymbolKind::BuiltinType, SymbolNameRef::Static("bool")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i8")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i16")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("i64")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("isize")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u8")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u16")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("u64")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("usize")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("f32")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("f64")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("String")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Slice")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Result")),
        (
            SymbolKind::BuiltinType,
            SymbolNameRef::Static("SyscallResult"),
        ),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Terminal")),
        (SymbolKind::BuiltinType, SymbolNameRef::Static("Never")),
    ]
}
