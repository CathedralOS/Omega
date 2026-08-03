use std::sync::Arc;

use psi_source::{SourceMap, SourceSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolName {
    storage: SymbolNameStorage,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum SymbolNameStorage {
    #[default]
    Missing,
    Source(SourceSpan),
    Static(&'static str),
    Owned(Arc<str>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SymbolNameStorageKind {
    #[default]
    Missing,
    Source,
    Static,
    Owned,
}

impl SymbolName {
    pub fn from_ref(name: SymbolNameRef<'_>) -> Self {
        Self {
            storage: match name {
                SymbolNameRef::Borrowed(value) => SymbolNameStorage::Owned(Arc::from(value)),
                SymbolNameRef::Source(source_span) => SymbolNameStorage::Source(source_span),
                SymbolNameRef::Static(value) => SymbolNameStorage::Static(value),
            },
        }
    }

    pub fn as_str<'source>(&'source self, sources: Option<&'source SourceMap>) -> &'source str {
        match &self.storage {
            SymbolNameStorage::Missing => "",
            SymbolNameStorage::Source(source_span) => sources
                .map(|sources| sources.text_at(*source_span))
                .unwrap_or(""),
            SymbolNameStorage::Static(value) => value,
            SymbolNameStorage::Owned(value) => value.as_ref(),
        }
    }

    pub fn storage_kind(&self) -> SymbolNameStorageKind {
        match self.storage {
            SymbolNameStorage::Missing => SymbolNameStorageKind::Missing,
            SymbolNameStorage::Source(_) => SymbolNameStorageKind::Source,
            SymbolNameStorage::Static(_) => SymbolNameStorageKind::Static,
            SymbolNameStorage::Owned(_) => SymbolNameStorageKind::Owned,
        }
    }

    pub fn source_span(&self) -> Option<SourceSpan> {
        match self.storage {
            SymbolNameStorage::Source(source_span) => Some(source_span),
            SymbolNameStorage::Missing
            | SymbolNameStorage::Static(_)
            | SymbolNameStorage::Owned(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolNameRef<'name> {
    Borrowed(&'name str),
    Source(SourceSpan),
    Static(&'static str),
}

impl<'name> SymbolNameRef<'name> {
    pub fn as_str(self) -> &'name str {
        match self {
            Self::Borrowed(value) => value,
            Self::Source(_) => "",
            Self::Static(value) => value,
        }
    }
}
