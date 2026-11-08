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
pub(crate) fn is_build_prelude_source(
    source: &source::SourceFile,
) -> Result<bool, Vec<diagnostics::Diagnostic>> {
    if source.origin != source::SourceOrigin::Toolchain
        || source.package_identity.is_some()
        || source.dependency_scope != source::DependencyScope::Build
        || source.resolution_stratum != source::SourceResolutionStratum::Base
        || source.path != std::path::Path::new(BUILD_PRELUDE_PATH)
    {
        return Ok(false);
    }
    // Standalone storage owns the complete library; package storage owns its
    // core dependency. Both issue this same virtual protocol unit. Neither
    // owner makes the unit a physical file in the captured core inventory.
    let library_root = crate::source::source_imports::bundled_omega_root()?;
    let core_root = crate::source::source_imports::bundled_core_root()?;
    Ok(source.package_root == library_root || source.package_root == core_root)
}

pub(super) const BUILD_PRELUDE_PATH: &str = "<build-prelude>";
