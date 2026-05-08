use crate::source::SourceSpan;

use super::{SymbolKind, SymbolNameRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDefinition<'name> {
    pub kind: SymbolKind,
    pub name: SymbolNameRef<'name>,
    pub debug_name: Option<SymbolNameRef<'name>>,
    pub children: Vec<SymbolDefinition<'name>>,
}

impl<'name> SymbolDefinition<'name> {
    pub fn named(kind: SymbolKind, name: &'name str) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Borrowed(name),
            debug_name: None,
            children: Vec::new(),
        }
    }

    pub fn static_named(kind: SymbolKind, name: &'static str) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Static(name),
            debug_name: None,
            children: Vec::new(),
        }
    }

    pub fn source_named(kind: SymbolKind, source_span: SourceSpan) -> Self {
        Self {
            kind,
            name: SymbolNameRef::Source(source_span),
            debug_name: None,
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
            debug_name: None,
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
            debug_name: None,
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
            debug_name: None,
            children: children.into_iter().collect(),
        }
    }

    pub fn with_debug_name(mut self, debug_name: &'name str) -> Self {
        self.debug_name = Some(SymbolNameRef::Borrowed(debug_name));
        self
    }

    pub fn with_static_debug_name(mut self, debug_name: &'static str) -> Self {
        self.debug_name = Some(SymbolNameRef::Static(debug_name));
        self
    }
}

pub fn builtin_type_symbol_definitions() -> [SymbolDefinition<'static>; 19] {
    [
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "bool"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "i8"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "i16"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "i32"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "i64"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "isize"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "u8"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "u16"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "u32"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "u64"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "usize"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "f32"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "f64"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "String"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "Slice"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "Result"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "SyscallResult"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "Terminal"),
        SymbolDefinition::static_named(SymbolKind::BuiltinType, "Never"),
    ]
}
