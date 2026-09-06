use std::sync::Arc;

use source::{SourceMap, SourceSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolName {
    storage: SymbolNameStorage,
}

#[derive(Clone, Default)]
enum SymbolNameStorage {
    #[default]
    Missing,
    Source(SourceSpan),
    SourceSlice {
        source: Arc<str>,
        source_span: SourceSpan,
    },
    OwnedSource {
        value: Arc<str>,
        source_span: SourceSpan,
    },
    Static(&'static str),
    Owned(Arc<str>),
}

impl SymbolNameStorage {
    fn retained_spelling(&self) -> Option<(&str, SourceSpan)> {
        match self {
            Self::SourceSlice {
                source,
                source_span,
            } => Some((
                &source[source_span.span.start..source_span.span.end],
                *source_span,
            )),
            Self::OwnedSource { value, source_span } => Some((value, *source_span)),
            _ => None,
        }
    }
}

impl PartialEq for SymbolNameStorage {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(left), Some(right)) = (self.retained_spelling(), other.retained_spelling()) {
            return left == right;
        }
        match (self, other) {
            (Self::Missing, Self::Missing) => true,
            (Self::Source(left), Self::Source(right)) => left == right,
            (Self::Static(left), Self::Static(right)) => left == right,
            (Self::Owned(left), Self::Owned(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for SymbolNameStorage {}

impl std::fmt::Debug for SymbolNameStorage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => formatter.write_str("Missing"),
            Self::Source(span) => formatter.debug_tuple("Source").field(span).finish(),
            Self::Static(value) => formatter.debug_tuple("Static").field(value).finish(),
            Self::Owned(value) => formatter.debug_tuple("Owned").field(value).finish(),
            Self::SourceSlice {
                source,
                source_span,
            } => formatter
                .debug_struct("OwnedSource")
                .field(
                    "value",
                    &&source[source_span.span.start..source_span.span.end],
                )
                .field("source_span", source_span)
                .finish(),
            Self::OwnedSource { value, source_span } => formatter
                .debug_struct("OwnedSource")
                .field("value", value)
                .field("source_span", source_span)
                .finish(),
        }
    }
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
                SymbolNameRef::OwnedSource { value, source_span } => {
                    SymbolNameStorage::OwnedSource {
                        value: Arc::from(value),
                        source_span,
                    }
                }
                SymbolNameRef::Static(value) => SymbolNameStorage::Static(value),
            },
        }
    }

    /// Reuses authored bytes only when they exactly match the semantic spelling.
    pub(crate) fn from_ref_with_sources(
        name: SymbolNameRef<'_>,
        sources: Option<&SourceMap>,
    ) -> Self {
        if let SymbolNameRef::OwnedSource { value, source_span } = name
            && let Some(file) = sources.and_then(|sources| sources.file_at(source_span))
            && file
                .source
                .get(source_span.span.start..source_span.span.end)
                == Some(value)
        {
            return Self {
                storage: SymbolNameStorage::SourceSlice {
                    source: file.source.clone(),
                    source_span,
                },
            };
        }
        Self::from_ref(name)
    }

    pub fn as_str<'source>(&'source self, sources: Option<&'source SourceMap>) -> &'source str {
        match &self.storage {
            SymbolNameStorage::Missing => "",
            SymbolNameStorage::Source(source_span) => sources
                .map(|sources| sources.text_at(*source_span))
                .unwrap_or(""),
            SymbolNameStorage::SourceSlice {
                source,
                source_span,
            } => &source[source_span.span.start..source_span.span.end],
            SymbolNameStorage::OwnedSource { value, .. } => value.as_ref(),
            SymbolNameStorage::Static(value) => value,
            SymbolNameStorage::Owned(value) => value.as_ref(),
        }
    }

    pub fn storage_kind(&self) -> SymbolNameStorageKind {
        match self.storage {
            SymbolNameStorage::Missing => SymbolNameStorageKind::Missing,
            SymbolNameStorage::Source(_) | SymbolNameStorage::SourceSlice { .. } => {
                SymbolNameStorageKind::Source
            }
            SymbolNameStorage::OwnedSource { .. } => SymbolNameStorageKind::Source,
            SymbolNameStorage::Static(_) => SymbolNameStorageKind::Static,
            SymbolNameStorage::Owned(_) => SymbolNameStorageKind::Owned,
        }
    }

    pub fn source_span(&self) -> Option<SourceSpan> {
        match self.storage {
            SymbolNameStorage::Source(source_span)
            | SymbolNameStorage::SourceSlice { source_span, .. } => Some(source_span),
            SymbolNameStorage::OwnedSource { source_span, .. } => Some(source_span),
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
    /// Owned semantic spelling attached to one exact authored declaration span.
    OwnedSource {
        value: &'name str,
        source_span: SourceSpan,
    },
    Static(&'static str),
}

impl<'name> SymbolNameRef<'name> {
    pub fn as_str(self) -> &'name str {
        match self {
            Self::Borrowed(value) => value,
            Self::Source(_) => "",
            Self::OwnedSource { value, .. } => value,
            Self::Static(value) => value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source::{SourceId, Span};

    #[test]
    fn retained_names_compare_and_format_only_the_selected_spelling() {
        let source_span = SourceSpan::new(SourceId(0), Span::new(0, 5));
        let seed = SymbolNameRef::OwnedSource {
            value: "Ready",
            source_span,
        };
        let owned = SymbolName::from_ref(seed);
        for text in ["Ready first", "Ready other"] {
            let mut sources = SourceMap::default();
            sources.add("main.omg".into(), text.into());
            let borrowed = SymbolName::from_ref_with_sources(seed, Some(&sources));
            assert_eq!(borrowed, owned);
            assert_eq!(format!("{borrowed:?}"), format!("{owned:?}"));
        }
    }

    #[test]
    fn source_reuse_requires_valid_exact_semantic_spelling() {
        let mut sources = SourceMap::default();
        let source_id = sources
            .add("main.omg".into(), "Ready \u{e9}".into())
            .source_id;
        for (source_span, value, should_borrow) in [
            (SourceSpan::new(source_id, Span::new(0, 5)), "Ready", true),
            (
                SourceSpan::new(source_id, Span::new(0, 5)),
                "Packet::Ready",
                false,
            ),
            (SourceSpan::new(source_id, Span::new(7, 8)), "", false),
            (SourceSpan::new(source_id, Span::new(0, 99)), "", false),
            (SourceSpan::new(SourceId(99), Span::new(0, 0)), "", false),
        ] {
            let name = SymbolName::from_ref_with_sources(
                SymbolNameRef::OwnedSource { value, source_span },
                Some(&sources),
            );
            assert_eq!(name.as_str(Some(&sources)), value);
            assert_eq!(name.source_span(), Some(source_span));
            assert_eq!(
                matches!(name.storage, SymbolNameStorage::SourceSlice { .. }),
                should_borrow
            );
        }
        let name = SymbolName::from_ref_with_sources(
            SymbolNameRef::OwnedSource {
                value: "Ready",
                source_span: SourceSpan::new(source_id, Span::new(0, 5)),
            },
            None,
        );
        assert_eq!(name.as_str(None), "Ready");
    }
}
