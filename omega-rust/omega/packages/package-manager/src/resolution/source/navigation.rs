//! Navigation from one authenticated acquisition root to one package root.

use package_source::SourceRelativePath;

/// Operational package location inside an acquired source.
///
/// This is replay/navigation custody. It never enters `PackageKey` or source
/// lineage identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageSourceNavigation {
    Root,
    Member(SourceRelativePath),
}

impl PackageSourceNavigation {}
