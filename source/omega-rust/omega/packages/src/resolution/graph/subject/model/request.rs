use super::super::super::ResolvedSourceIdentity;
use crate::declarations::BuildDeclarationKind;
use crate::declarations::dependencies::read::{DependencySourceRequest, PackageSelection};
use crate::declarations::{AliasName, PackageKey, PackageName};
use omega_package_source::{ExternalSourceContext, SourceLineage, SourceRelativePath};

/// Exact caller request for the root source, before normalized selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalRootSourceRequest {
    Git {
        requested_locator: String,
        requested_revision: String,
        selection: PackageSelection,
    },
    WorkspaceMember {
        workspace_root_source: SourceLineage,
        member_path: SourceRelativePath,
        /// Exact platform-encoded caller spelling. This is not a cache path.
        requested_workspace_root: Vec<u8>,
    },
    ExternalLocal {
        /// Exact platform-encoded caller spelling. Canonical local lineage is
        /// retained independently in the selected package key.
        requested_root: Vec<u8>,
        source_context: ExternalSourceContext,
    },
}

/// One exact root request joined directly to the immutable source it selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalRootSourceSelection {
    pub(in super::super) request: CanonicalRootSourceRequest,
    pub(in super::super) role: BuildDeclarationKind,
    pub(in super::super) selected: ResolvedSourceIdentity,
}

impl CanonicalRootSourceSelection {
    pub const fn request(&self) -> &CanonicalRootSourceRequest {
        &self.request
    }

    pub const fn selected(&self) -> &ResolvedSourceIdentity {
        &self.selected
    }

    pub const fn role(&self) -> BuildDeclarationKind {
        self.role
    }
}

/// Exact authored source request for one dependency occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalDependencySourceRequest {
    Path {
        explicit_alias: Option<AliasName>,
        location: String,
    },
    Git {
        explicit_alias: Option<AliasName>,
        repository: String,
        revision: String,
        selection: PackageSelection,
    },
}

impl CanonicalDependencySourceRequest {
    pub const fn explicit_alias(&self) -> Option<&AliasName> {
        match self {
            Self::Path { explicit_alias, .. } | Self::Git { explicit_alias, .. } => {
                explicit_alias.as_ref()
            }
        }
    }

    pub(in super::super) fn resolved_alias(&self, selected: &PackageName) -> AliasName {
        self.explicit_alias()
            .cloned()
            .unwrap_or_else(|| selected.default_alias())
    }
}

impl From<&DependencySourceRequest> for CanonicalDependencySourceRequest {
    fn from(request: &DependencySourceRequest) -> Self {
        match request {
            DependencySourceRequest::Path {
                explicit_alias,
                location,
            } => Self::Path {
                explicit_alias: explicit_alias.clone(),
                location: location.clone(),
            },
            DependencySourceRequest::Git {
                explicit_alias,
                repository,
                revision,
                selection,
            } => Self::Git {
                explicit_alias: explicit_alias.clone(),
                repository: repository.clone(),
                revision: revision.clone(),
                selection: selection.clone(),
            },
        }
    }
}

/// One requester-owned dependency request joined to its graph edge and exact
/// immutable selection. Distinct diamond occurrences remain distinct rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalDependencySourceSelection {
    pub(in super::super) requester: PackageKey,
    pub(in super::super) dependency_index: usize,
    pub(in super::super) request: CanonicalDependencySourceRequest,
    pub(in super::super) alias: AliasName,
    pub(in super::super) selected: ResolvedSourceIdentity,
}

impl CanonicalDependencySourceSelection {
    pub const fn requester(&self) -> &PackageKey {
        &self.requester
    }

    pub const fn dependency_index(&self) -> usize {
        self.dependency_index
    }

    pub const fn request(&self) -> &CanonicalDependencySourceRequest {
        &self.request
    }

    pub const fn alias(&self) -> &AliasName {
        &self.alias
    }

    pub const fn selected(&self) -> &ResolvedSourceIdentity {
        &self.selected
    }
}
