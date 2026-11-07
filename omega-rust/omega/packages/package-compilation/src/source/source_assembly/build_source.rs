// `BuildSourceSelection` and `<build-prelude>` source identity.
//
// Only source assembly can mint an `Authored` or `CompilerDefault` `BuildSourceSelection`,
// and consumers track that choice across source rejoins via `rejoined(source_id)`.
// `is_build_prelude_source` verifies that a `<build-prelude>` `SourceFile` carries
// `Toolchain` origin, no package identity, `Build` dependency scope, `Base` resolution
// stratum, and either the bundled `omega` library root (standalone) or `core` root
// (package-aware), preventing a user file with the same diagnostic spelling from
// impersonating the compiler's build prelude.

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
    let library_root = crate::source::source_imports::bundled_omega_root()?;
    let core_root = crate::source::source_imports::bundled_core_root()?;
    Ok(source.package_root == library_root || source.package_root == core_root)
}

pub(super) const BUILD_PRELUDE_PATH: &str = "<build-prelude>";
