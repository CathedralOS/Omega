use symbols::{
    BuiltinType, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder,
    builtin_type_member_symbols,
};

pub(in crate::symbols::symbol_table) fn insert_builtin_type_symbol_children(
    builder: &mut SymbolTableBuilder,
    builtin_symbol: SymbolHandle,
    builtin_type: (SymbolKind, SymbolNameRef<'static>),
) {
    let SymbolNameRef::Static(name) = builtin_type.1 else {
        return;
    };
    let Some(builtin_type) = BuiltinType::from_name(name) else {
        return;
    };

    builder.insert_children(builtin_symbol, builtin_type_member_symbols(builtin_type));
}
