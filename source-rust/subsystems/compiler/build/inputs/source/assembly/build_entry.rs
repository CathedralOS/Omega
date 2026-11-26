// `BuildSourceSelection` and `<build-prelude>` source identity.
//
// Only source assembly can mint an `Authored` or `CompilerDefault` `BuildSourceSelection`,
// and consumers track that choice across source rejoins via `rejoined(source_id)`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub struct BuildSourceSelection {
    entry: source_text::SourceSpan,
    kind: BuildSourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
enum BuildSourceKind {
    Authored,
    CompilerDefault,
}

impl BuildSourceSelection {
    pub fn source_id(self) -> source_text::SourceId {
        self.entry.source_id
    }
    pub fn entry(self) -> source_text::SourceSpan {
        self.entry
    }
    pub fn is_default(self) -> bool {
        self.kind == BuildSourceKind::CompilerDefault
    }
    pub(super) fn authored(entry: source_text::SourceSpan) -> Self {
        Self {
            entry,
            kind: BuildSourceKind::Authored,
        }
    }
    pub(super) fn compiler_default(entry: source_text::SourceSpan) -> Self {
        Self {
            entry,
            kind: BuildSourceKind::CompilerDefault,
        }
    }
    pub fn rejoined(self, source_id: source_text::SourceId) -> Self {
        Self {
            entry: source_text::SourceSpan::new(source_id, self.entry.span),
            ..self
        }
    }
}
