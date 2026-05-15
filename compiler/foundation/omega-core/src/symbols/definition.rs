use crate::source::SourceSpan;

use super::{SymbolKind, SymbolNameRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDefinition<'name> {
    pub kind: SymbolKind,
    pub name: SymbolNameRef<'name>,
    pub children: Vec<SymbolDefinition<'name>>,
}

impl<'name> SymbolDefinition<'name> {
    pub fn named(kind: SymbolKind, name: &'name str) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Borrowed(name),
            children: Vec::new(),
        }
    }

    pub fn static_named(kind: SymbolKind, name: &'static str) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Static(name),
            children: Vec::new(),
        }
    }

    pub fn source_named(kind: SymbolKind, source_span: SourceSpan) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Source(source_span),
            children: Vec::new(),
        }
    }

    pub fn with_children(
        kind: SymbolKind,
        name: &'name str,
        children: impl IntoIterator<Item = SymbolDefinition<'name>>,
    ) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Borrowed(name),
            children: children.into_iter().collect(),
        }
    }

    pub fn static_with_children(
        kind: SymbolKind,
        name: &'static str,
        children: impl IntoIterator<Item = SymbolDefinition<'name>>,
    ) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Static(name),
            children: children.into_iter().collect(),
        }
    }

    pub fn source_with_children(
        kind: SymbolKind,
        source_span: SourceSpan,
        children: impl IntoIterator<Item = SymbolDefinition<'name>>,
    ) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Source(source_span),
            children: children.into_iter().collect(),
        }
    }
}

pub fn builtin_type_symbol_definitions() -> [SymbolDefinition<'static>; 19] {
    builtin_type_symbols().map(|(kind, name)| SymbolDefinition {
        kind,
        name,
        children: Vec::new(),
    })
}

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
