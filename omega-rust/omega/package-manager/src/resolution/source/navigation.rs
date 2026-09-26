//! Navigation from one authenticated acquisition root to one package root.

use package_source::SourceRelativePath;

/// Operational package location inside an acquired source.
///
/// This is replay/navigation custody. It never enters `PackageKey` or source
/// lineage identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceNavigation {
    Root,
    Member(SourceRelativePath),
}

impl PackageSourceNavigation {
    pub const fn member_path(&self) -> Option<&SourceRelativePath> {
        match self {
            Self::Root => None,
            Self::Member(path) => Some(path),
        }
    }
}
