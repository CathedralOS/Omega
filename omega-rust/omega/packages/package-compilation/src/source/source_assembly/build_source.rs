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

/// This role is issued by source assembly, never by an authored path. The
/// origin, scope and visibility checks distinguish the compiler's protocol
/// unit from a user file with the same diagnostic spelling.
pub(crate) fn is_build_prelude_source(source: &source::SourceFile) -> bool {
    source.origin == source::SourceOrigin::Toolchain
        && source.package_identity.is_none()
        && source.dependency_scope == source::DependencyScope::Build
        && source.resolution_stratum == source::SourceResolutionStratum::Base
        && source.path == std::path::Path::new(BUILD_PRELUDE_PATH)
}

pub(super) const BUILD_PRELUDE_PATH: &str = "<build-prelude>";
