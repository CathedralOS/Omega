//! Exact bytes published for one selected package root.

use package_source::{ResolvedLocalSource, SourceContentDigest};

/// Recheckable commitment to the package subtree exposed to compilation.
///
/// Git repository resolution and package materialization are distinct: several
/// selected members may share one commit/root tree while exposing different
/// immutable package subtrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceMaterialization {
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

    #[cfg(test)]
    pub(crate) fn synthetic(content: SourceContentDigest) -> Self {
        Self {
            content,
            file_count: 0,
            byte_count: 0,
        }
    }

    pub const fn content(&self) -> &SourceContentDigest {
        &self.content
    }

    pub const fn file_count(&self) -> usize {
        self.file_count
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }
}
