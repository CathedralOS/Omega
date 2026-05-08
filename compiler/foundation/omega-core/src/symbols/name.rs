use crate::source::{SourceMap, SourceSpan};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SymbolName {
    #[default]
    Missing,
    Source(SourceSpan),
    Static(&'static str),
    Owned(String),
}

impl SymbolName {
    pub fn from_ref(name: SymbolNameRef<'_>) -> Self {
        match name {
            SymbolNameRef::Borrowed(value) => Self::Owned(value.to_owned()),
            SymbolNameRef::Source(source_span) => Self::Source(source_span),
            SymbolNameRef::Static(value) => Self::Static(value),
        }
    }

    pub fn as_str<'source>(&'source self, sources: Option<&'source SourceMap>) -> &'source str {
        match self {
            Self::Missing => "",
            Self::Source(source_span) => sources
                .map(|sources| sources.text_at(*source_span))
                .unwrap_or(""),
            Self::Static(value) => value,
            Self::Owned(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolDebugName {
    pub value: String,
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
