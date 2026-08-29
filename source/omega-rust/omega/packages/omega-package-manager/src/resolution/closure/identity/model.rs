//! Canonical closure-identity model and construction.

use super::super::{ResolvedPackageSourceClosure, ResolvedSourceIdentity};
use crate::manifest::dependencies::read::{DependencySourceRequest, PackageSelection};
use omega_package_source::{
    AliasName, ExternalSourceContext, PackageKey, PackageName, SourceLineage, WorkspaceMemberPath,
};
use std::fmt;

#[path = "encoding.rs"]
mod encoding;
#[path = "validation.rs"]
mod validation;

use encoding::{
    Decoder, decode_dependency_selection, decode_root_selection, decode_source_identity,
    encode_hex, encode_subject, fingerprint,
};
use validation::{canonical_root_request, validate_subject};

const SOURCE_CLOSURE_SUBJECT_MAGIC: &[u8] = b"OMEGA-SOURCE-CLOSURE-SUBJECT\0";
pub const SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION: u16 = 2;
const SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN: &[u8] =
    b"OMEGA-SOURCE-CLOSURE-SUBJECT-FINGERPRINT\0";
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

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
    fn compiler_bounded(self) -> Self {
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

/// A closed error from projection or strict canonical recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubjectError {
    message: &'static str,
}

impl CanonicalSourceClosureSubjectError {
    fn new(message: &'static str) -> Self {
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
pub struct CanonicalSourceClosureSubjectFingerprint([u8; 32]);

impl CanonicalSourceClosureSubjectFingerprint {
    pub fn to_hex(&self) -> String {
        encode_hex(&self.0)
    }
}

/// Exact caller request for the root source, before normalized selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalRootSourceRequest {
    Git {
        requested_locator: String,
        requested_revision: String,
    },
    WorkspaceMember {
        workspace_root_source: SourceLineage,
        member_path: WorkspaceMemberPath,
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
    request: CanonicalRootSourceRequest,
    selected: ResolvedSourceIdentity,
}

impl CanonicalRootSourceSelection {
    pub const fn request(&self) -> &CanonicalRootSourceRequest {
        &self.request
    }

    pub const fn selected(&self) -> &ResolvedSourceIdentity {
        &self.selected
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

    fn resolved_alias(&self, selected: &PackageName) -> AliasName {
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
    requester: PackageKey,
    dependency_index: usize,
    request: CanonicalDependencySourceRequest,
    alias: AliasName,
    selected: ResolvedSourceIdentity,
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

/// Canonical, non-admitting subject for one exact resolved source closure.
///
/// The subject binds every package key and immutable resolution, the exact root
/// request, and every requester-local dependency request occurrence. Snapshot
/// roots, cache paths, transport execution observations, compiler evidence,
/// certificates, decisions, and artifacts are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubject {
    root: CanonicalRootSourceSelection,
    packages: Vec<ResolvedSourceIdentity>,
    dependency_requests: Vec<CanonicalDependencySourceSelection>,
    canonical_bytes: Vec<u8>,
    fingerprint: CanonicalSourceClosureSubjectFingerprint,
}

impl CanonicalSourceClosureSubject {
    pub fn from_resolved(
        closure: &ResolvedPackageSourceClosure,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let root_view = closure.source_requests().root();
        let root = CanonicalRootSourceSelection {
            request: canonical_root_request(root_view.request()),
            selected: root_view.selected().clone(),
        };
        let mut packages = closure
            .graph()
            .packages()
            .iter()
            .map(|package| package.source().clone())
            .collect::<Vec<_>>();
        packages.sort_by(|left, right| left.key().cmp(right.key()));
        let mut dependency_requests = closure
            .source_requests()
            .dependencies()
            .map(|selection| CanonicalDependencySourceSelection {
                requester: selection.requester().clone(),
                dependency_index: selection.dependency_index(),
                request: CanonicalDependencySourceRequest::from(selection.request()),
                alias: selection.alias().clone(),
                selected: selection.selected().clone(),
            })
            .collect::<Vec<_>>();
        dependency_requests.sort_by(|left, right| {
            left.requester
                .cmp(&right.requester)
                .then(left.dependency_index.cmp(&right.dependency_index))
        });
        Self::finish(root, packages, dependency_requests, limits)
    }

    pub fn recover(
        bytes: &[u8],
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let limits = limits.compiler_bounded();
        if bytes.len() > limits.maximum_record_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject exceeds its record-byte limit",
            ));
        }
        let mut decoder = Decoder::new(bytes);
        decoder.expect_fixed(SOURCE_CLOSURE_SUBJECT_MAGIC)?;
        if decoder.u16()? != SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION {
            return Err(CanonicalSourceClosureSubjectError::new(
                "unsupported source-closure subject version",
            ));
        }
        let root = decode_root_selection(&mut decoder, limits)?;
        let package_count = decoder.count(limits.maximum_packages)?;
        let mut packages = Vec::with_capacity(package_count);
        for _ in 0..package_count {
            packages.push(decode_source_identity(
                &mut decoder,
                limits.maximum_identity_bytes,
            )?);
        }
        let request_count = decoder.count(limits.maximum_dependency_requests)?;
        let mut dependency_requests = Vec::with_capacity(request_count);
        for _ in 0..request_count {
            dependency_requests.push(decode_dependency_selection(&mut decoder, limits)?);
        }
        decoder.finish()?;
        let recovered = Self::finish(root, packages, dependency_requests, limits)?;
        if recovered.canonical_bytes != bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject is not canonically encoded",
            ));
        }
        Ok(recovered)
    }

    pub const fn root(&self) -> &CanonicalRootSourceSelection {
        &self.root
    }

    pub fn packages(&self) -> &[ResolvedSourceIdentity] {
        &self.packages
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

    /// Require a recovered subject to equal a fresh projection from the exact
    /// current resolver custody. Decode or fingerprint equality alone is never
    /// enough for this comparison.
    pub fn matches_resolved(
        &self,
        closure: &ResolvedPackageSourceClosure,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<bool, CanonicalSourceClosureSubjectError> {
        Ok(self == &Self::from_resolved(closure, limits)?)
    }

    fn finish(
        root: CanonicalRootSourceSelection,
        packages: Vec<ResolvedSourceIdentity>,
        dependency_requests: Vec<CanonicalDependencySourceSelection>,
        limits: CanonicalSourceClosureSubjectLimits,
    ) -> Result<Self, CanonicalSourceClosureSubjectError> {
        let limits = limits.compiler_bounded();
        validate_subject(&root, &packages, &dependency_requests, limits)?;
        let canonical_bytes = encode_subject(&root, &packages, &dependency_requests, limits)?;
        if canonical_bytes.len() > limits.maximum_record_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject exceeds its record-byte limit",
            ));
        }
        let fingerprint = fingerprint(&canonical_bytes);
        Ok(Self {
            root,
            packages,
            dependency_requests,
            canonical_bytes,
            fingerprint,
        })
    }
}
