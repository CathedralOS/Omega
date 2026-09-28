//! Canonical, recoverable subject of one complete resolved source closure.
//!
//! [`CanonicalSourceClosureSubject`] and the requests, selections, limits,
//! fingerprint and error it is made of are defined here; `construction`
//! builds one from an exact resolved closure, `encoding` writes and reads
//! its canonical bytes, `validation` replays it against its limits,
//! `request_view` and `text` present it, and `usage` accounts the budget a
//! recovery spends.

mod construction;
mod encoding;
mod request_view;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod text;
mod usage;
mod validation;

pub use usage::CanonicalSourceClosureSubjectRecoveryUsage;

use std::fmt;

use package_source::{ExternalSourceContext, SourceLineage, SourceRelativePath};
use target::TargetProfile;

use super::ResolvedSourceIdentity;
use crate::declarations::dependencies::read::{
    DependencyProjections, DependencyPurpose, DependencySourceRequest, PackageSelection,
};
use crate::declarations::{AliasName, BuildDeclarationKind, PackageKey};
use crate::resolution::source::PackageSourceNavigation;
use encoding::encode_hex;

pub(super) const SOURCE_CLOSURE_SUBJECT_MAGIC: &[u8] = b"OMEGA-SOURCE-CLOSURE-SUBJECT\0";
/// Version of the binary subject encoding.
///
/// v7 records each dependency edge's authorized purpose (product or build)
/// and splits each package's authored requests by purpose. v6 subjects have
/// only product-purpose requests and edges; they are not accepted as v7
/// records and must be re-projected through the versioned lock migration.
pub const SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION: u16 = 7;
pub(super) const SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN: &[u8] =
    b"OMEGA-SOURCE-CLOSURE-SUBJECT-FINGERPRINT\0";

/// Canonical, non-admitting subject for one exact resolved source closure.
///
/// The subject binds every package key and immutable resolution, the exact root
/// request, and every requester-local dependency request occurrence. Snapshot
/// roots, cache paths, transport execution observations, compiler evidence,
/// certificates, decisions, and artifacts are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubject {
    pub(super) target_profile: TargetProfile,
    pub(super) root: CanonicalRootSourceSelection,
    pub(super) packages: Vec<ResolvedSourceIdentity>,
    pub(super) package_navigations: Vec<PackageSourceNavigation>,
    pub(super) package_dependency_projections: Vec<DependencyProjections>,
    pub(super) dependency_requests: Vec<CanonicalDependencySourceSelection>,
    pub(super) canonical_bytes: Vec<u8>,
    pub(super) fingerprint: CanonicalSourceClosureSubjectFingerprint,
}

impl CanonicalSourceClosureSubject {
    /// Compare every target-independent source field without allocation.
    /// Target-scoped encoded bytes and fingerprints are deliberately excluded.
    pub fn same_source_graph(&self, other: &Self) -> bool {
        self.root == other.root
            && self.packages == other.packages
            && self.package_navigations == other.package_navigations
            && self.package_dependency_projections == other.package_dependency_projections
            && self.dependency_requests == other.dependency_requests
    }

    pub const fn root(&self) -> &CanonicalRootSourceSelection {
        &self.root
    }

    pub const fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }

    pub const fn root_role(&self) -> BuildDeclarationKind {
        self.root.role()
    }

    pub fn packages(&self) -> &[ResolvedSourceIdentity] {
        &self.packages
    }

    pub fn package_navigation(&self, package: &PackageKey) -> Option<&PackageSourceNavigation> {
        self.packages
            .binary_search_by(|source| source.key().cmp(package))
            .ok()
            .map(|index| &self.package_navigations[index])
    }

    pub fn package_dependency_projection(
        &self,
        package: &PackageKey,
    ) -> Option<&DependencyProjections> {
        self.packages
            .binary_search_by(|source| source.key().cmp(package))
            .ok()
            .map(|index| &self.package_dependency_projections[index])
    }

    pub fn dependency_requests(&self) -> &[CanonicalDependencySourceSelection] {
        &self.dependency_requests
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn fingerprint(&self) -> &CanonicalSourceClosureSubjectFingerprint {
        &self.fingerprint
    }
}

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
    pub(super) request: CanonicalRootSourceRequest,
    pub(super) role: BuildDeclarationKind,
    pub(super) selected: ResolvedSourceIdentity,
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
    pub(super) requester: PackageKey,
    pub(super) purpose: DependencyPurpose,
    pub(super) dependency_index: usize,
    pub(super) request: CanonicalDependencySourceRequest,
    pub(super) alias: AliasName,
    pub(super) selected: ResolvedSourceIdentity,
}

impl CanonicalDependencySourceSelection {
    pub const fn requester(&self) -> &PackageKey {
        &self.requester
    }

    /// Which authorized context this edge belongs to.
    pub const fn purpose(&self) -> DependencyPurpose {
        self.purpose
    }

    /// Zero-based position in the requester's authored rows for this edge's
    /// purpose scope.
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

/// A closed error from projection or strict canonical recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubjectError {
    message: &'static str,
}

impl CanonicalSourceClosureSubjectError {
    pub(super) fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for CanonicalSourceClosureSubjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for CanonicalSourceClosureSubjectError {}

/// Domain-separated identity of one complete canonical source-closure question.
///
/// This identifies the question only. It is not source authenticity, package
/// admission, a compiler result, or a package instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalSourceClosureSubjectFingerprint(pub(super) [u8; 32]);

impl CanonicalSourceClosureSubjectFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        encode_hex(&self.0)
    }
}

const ABSOLUTE_RECORD_BYTE_LIMIT: usize = 64 * 1024 * 1024;
const ABSOLUTE_PACKAGE_LIMIT: usize = 16 * 1024;
const ABSOLUTE_DEPENDENCY_REQUEST_LIMIT: usize = 256 * 1024;
const ABSOLUTE_IDENTITY_BYTE_LIMIT: usize = 1024 * 1024;
const ABSOLUTE_REQUEST_BYTE_LIMIT: usize = 1024 * 1024;

/// Resource ceilings for one canonical resolved-source question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubjectLimits {
    pub maximum_record_bytes: usize,
    pub maximum_packages: usize,
    pub maximum_dependency_requests: usize,
    pub maximum_identity_bytes: usize,
    pub maximum_request_bytes: usize,
}

impl Default for CanonicalSourceClosureSubjectLimits {
    fn default() -> Self {
        Self {
            maximum_record_bytes: ABSOLUTE_RECORD_BYTE_LIMIT,
            maximum_packages: 1024,
            maximum_dependency_requests: 16 * 1024,
            maximum_identity_bytes: 64 * 1024,
            maximum_request_bytes: 64 * 1024,
        }
    }
}

impl CanonicalSourceClosureSubjectLimits {
    pub(super) fn compiler_bounded(self) -> Self {
        Self {
            maximum_record_bytes: self.maximum_record_bytes.min(ABSOLUTE_RECORD_BYTE_LIMIT),
            maximum_packages: self.maximum_packages.min(ABSOLUTE_PACKAGE_LIMIT),
            maximum_dependency_requests: self
                .maximum_dependency_requests
                .min(ABSOLUTE_DEPENDENCY_REQUEST_LIMIT),
            maximum_identity_bytes: self
                .maximum_identity_bytes
                .min(ABSOLUTE_IDENTITY_BYTE_LIMIT),
            maximum_request_bytes: self.maximum_request_bytes.min(ABSOLUTE_REQUEST_BYTE_LIMIT),
        }
    }
}
