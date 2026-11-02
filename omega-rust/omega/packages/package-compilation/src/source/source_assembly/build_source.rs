//! Source assembly alone selects authored code or the ordinary default template.
//! Consumers retain this choice with its exact source instance through rejoins;
//! neither a machine name nor an optional role result can grant default semantics.

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct BuildSourceSelection {
    source_id: source::SourceId,
    kind: BuildSourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
enum BuildSourceKind {
    Authored,
    CompilerDefault,
}

impl BuildSourceSelection {
    pub fn source_id(self) -> source::SourceId {
        self.source_id
    }
    pub fn is_default(self) -> bool {
        self.kind == BuildSourceKind::CompilerDefault
    }
    pub(super) fn authored(source_id: source::SourceId) -> Self {
        Self {
            source_id,
            kind: BuildSourceKind::Authored,
        }
    }
    pub(super) fn compiler_default(source_id: source::SourceId) -> Self {
        Self {
            source_id,
            kind: BuildSourceKind::CompilerDefault,
        }
    }
    pub(super) fn rejoined(self, source_id: source::SourceId) -> Self {
        Self { source_id, ..self }
    }
}
