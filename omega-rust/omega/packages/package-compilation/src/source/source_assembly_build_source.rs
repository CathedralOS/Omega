// `BuildSourceSelection` and `<build-prelude>` source identity.
//
// Only source assembly can mint an `Authored` or `CompilerDefault` `BuildSourceSelection`,
// and consumers track that choice across source rejoins via `rejoined(source_id)`.

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

pub(super) const BUILD_PRELUDE_PATH: &str = "<build-prelude>";
