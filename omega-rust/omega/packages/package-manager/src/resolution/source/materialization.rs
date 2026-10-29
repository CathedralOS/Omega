//! Exact bytes published for one selected package root.

use package_source::{ResolvedLocalSource, SourceContentDigest};

/// Recheckable commitment to the package subtree exposed to compilation.
///
/// Git repository resolution and package materialization are distinct: several
/// selected members may share one commit/root tree while exposing different
/// immutable package subtrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageSourceMaterialization {
    content: SourceContentDigest,
    file_count: usize,
    byte_count: u64,
}

impl PackageSourceMaterialization {
    pub(crate) fn from_local(source: &ResolvedLocalSource) -> Self {
        Self {
            content: SourceContentDigest::derive(source.content_identity.as_bytes()),
            file_count: source.file_count,
            byte_count: source.byte_count,
        }
    }

    pub const fn content(&self) -> &SourceContentDigest {
        &self.content
    }
}
