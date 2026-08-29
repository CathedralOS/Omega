//! Canonical, recoverable identity of one complete resolved source closure.

use crate::declarations::dependency_projection::DependencySourceRequest;
use crate::resolution::closure_resolution::{
    PackageRootSourceRequest, ResolvedPackageSourceClosure,
};
use crate::resolution::graph::{
    ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode, ResolvedSourceIdentity,
};
use crate::resolution::identity::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitTransport, GitTreeId,
    ImmutableSourceResolution, PackageKey, PackageName, SourceContentDigest, SourceLineage,
    WorkspaceLineageIdentity, WorkspaceMemberLineage, WorkspaceMemberPath,
};
use crate::source::GitSourceRequest;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

const SOURCE_CLOSURE_SUBJECT_MAGIC: &[u8] = b"OMEGA-SOURCE-CLOSURE-SUBJECT\0";
pub const SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION: u16 = 1;
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
            } => Self::Git {
                explicit_alias: explicit_alias.clone(),
                repository: repository.clone(),
                revision: revision.clone(),
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

fn canonical_root_request(request: &PackageRootSourceRequest) -> CanonicalRootSourceRequest {
    match request {
        PackageRootSourceRequest::Git(request) => CanonicalRootSourceRequest::Git {
            requested_locator: request.requested_locator().to_owned(),
            requested_revision: request.requested_revision().to_owned(),
        },
        PackageRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source: workspace_root_source.clone(),
            member_path: member_path.clone(),
            requested_workspace_root: requested_workspace_root
                .as_os_str()
                .as_encoded_bytes()
                .to_vec(),
        },
        PackageRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => CanonicalRootSourceRequest::ExternalLocal {
            requested_root: requested_root.as_os_str().as_encoded_bytes().to_vec(),
            source_context: source_context.clone(),
        },
    }
}

fn validate_subject(
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    dependency_requests: &[CanonicalDependencySourceSelection],
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if packages.is_empty() || packages.len() > limits.maximum_packages {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source-closure subject violates its package-count limit",
        ));
    }
    if dependency_requests.len() > limits.maximum_dependency_requests {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source-closure subject violates its request-count limit",
        ));
    }
    for source in packages {
        validate_source_identity(source, limits.maximum_identity_bytes)?;
    }
    if packages
        .windows(2)
        .any(|pair| pair[0].key() >= pair[1].key())
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source-closure packages are not in strict canonical order",
        ));
    }
    validate_source_identity(&root.selected, limits.maximum_identity_bytes)?;
    let package_by_key = packages
        .iter()
        .map(|source| (source.key(), source))
        .collect::<BTreeMap<_, _>>();
    if package_by_key.get(root.selected.key()).copied() != Some(&root.selected) {
        return Err(CanonicalSourceClosureSubjectError::new(
            "root request selection is absent or resolution-mismatched",
        ));
    }
    validate_root_request(root, limits)?;

    let mut previous: Option<(&PackageKey, usize)> = None;
    let mut dependencies = BTreeMap::<PackageKey, Vec<ResolvedDependency>>::new();
    for selection in dependency_requests {
        validate_source_identity(&selection.selected, limits.maximum_identity_bytes)?;
        validate_dependency_request(&selection.request, limits.maximum_request_bytes)?;
        if package_by_key.get(&selection.requester).is_none() {
            return Err(CanonicalSourceClosureSubjectError::new(
                "dependency request names an unknown requester",
            ));
        }
        if package_by_key.get(selection.selected.key()).copied() != Some(&selection.selected) {
            return Err(CanonicalSourceClosureSubjectError::new(
                "dependency request selection is absent or resolution-mismatched",
            ));
        }
        if selection.alias
            != selection
                .request
                .resolved_alias(selection.selected.key().name())
        {
            return Err(CanonicalSourceClosureSubjectError::new(
                "dependency request alias disagrees with its authored selection",
            ));
        }
        match previous {
            Some((requester, previous_index)) if requester == &selection.requester => {
                if selection.dependency_index != previous_index + 1 {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "dependency request ordinals are not contiguous",
                    ));
                }
            }
            Some((requester, _)) if requester >= &selection.requester => {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency requests are not in strict canonical order",
                ));
            }
            _ if selection.dependency_index != 0 => {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency request ordinals do not begin at zero",
                ));
            }
            _ => {}
        }
        validate_dependency_selection_kind(selection)?;
        dependencies
            .entry(selection.requester.clone())
            .or_default()
            .push(ResolvedDependency::new(
                selection.alias.clone(),
                selection.selected.key().clone(),
            ));
        previous = Some((&selection.requester, selection.dependency_index));
    }

    let nodes = packages
        .iter()
        .map(|source| {
            ResolvedPackageNode::new(
                source.clone(),
                dependencies.remove(source.key()).unwrap_or_default(),
            )
        })
        .collect();
    ResolvedPackageClosure::new(root.selected.key().clone(), nodes).map_err(|_| {
        CanonicalSourceClosureSubjectError::new(
            "source-closure subject does not form one closed reachable acyclic graph",
        )
    })?;
    Ok(())
}

fn validate_root_request(
    root: &CanonicalRootSourceSelection,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match &root.request {
        CanonicalRootSourceRequest::Git {
            requested_locator,
            requested_revision,
        } => {
            validate_request_bytes(requested_locator.as_bytes(), limits.maximum_request_bytes)?;
            validate_request_bytes(requested_revision.as_bytes(), limits.maximum_request_bytes)?;
            let request =
                GitSourceRequest::new(requested_locator.clone(), Some(requested_revision.clone()))
                    .map_err(|_| {
                        CanonicalSourceClosureSubjectError::new("invalid root Git request")
                    })?;
            if request.lineage() != root.selected.key().source_lineage()
                || !matches!(
                    root.selected.resolution(),
                    ImmutableSourceResolution::Git { .. }
                )
            {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "root Git request disagrees with its selected source",
                ));
            }
        }
        CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => {
            validate_source_lineage(workspace_root_source, limits.maximum_identity_bytes)?;
            validate_request_bytes(
                member_path.as_str().as_bytes(),
                limits.maximum_request_bytes,
            )?;
            validate_request_bytes(requested_workspace_root, limits.maximum_request_bytes)?;
            let identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new(
                        "invalid workspace root source in root request",
                    )
                })?;
            if !matches!(
                root.selected.key().source_lineage(),
                SourceLineage::Workspace(lineage)
                    if lineage.workspace_identity() == &identity
                        && lineage.member_path() == member_path
            ) || !matches!(
                root.selected.resolution(),
                ImmutableSourceResolution::Workspace { .. }
            ) {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "workspace root request disagrees with its selected source",
                ));
            }
        }
        CanonicalRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => {
            validate_request_bytes(requested_root, limits.maximum_request_bytes)?;
            if !matches!(
                root.selected.key().source_lineage(),
                SourceLineage::ExternalLocal(lineage)
                    if lineage.source_context() == source_context
            ) || !matches!(
                root.selected.resolution(),
                ImmutableSourceResolution::ExternalLocal { .. }
            ) {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "external-local root request disagrees with its selected source",
                ));
            }
        }
    }
    Ok(())
}

fn validate_dependency_selection_kind(
    selection: &CanonicalDependencySourceSelection,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match &selection.request {
        CanonicalDependencySourceRequest::Path { .. } => {
            if !matches!(
                selection.selected.key().source_lineage(),
                SourceLineage::Workspace(_) | SourceLineage::ExternalLocal(_)
            ) {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "path request selected a non-path source lineage",
                ));
            }
        }
        CanonicalDependencySourceRequest::Git {
            repository,
            revision,
            ..
        } => {
            let request = GitSourceRequest::new(repository.clone(), Some(revision.clone()))
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid dependency Git request")
                })?;
            if request.lineage() != selection.selected.key().source_lineage()
                || !matches!(
                    selection.selected.resolution(),
                    ImmutableSourceResolution::Git { .. }
                )
            {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency Git request disagrees with its selected source",
                ));
            }
        }
    }
    Ok(())
}

fn validate_dependency_request(
    request: &CanonicalDependencySourceRequest,
    maximum_request_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match request {
        CanonicalDependencySourceRequest::Path {
            explicit_alias,
            location,
        } => {
            validate_optional_alias(explicit_alias)?;
            validate_request_bytes(location.as_bytes(), maximum_request_bytes)
        }
        CanonicalDependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
        } => {
            validate_optional_alias(explicit_alias)?;
            validate_request_bytes(repository.as_bytes(), maximum_request_bytes)?;
            validate_request_bytes(revision.as_bytes(), maximum_request_bytes)
        }
    }
}

fn validate_optional_alias(
    alias: &Option<AliasName>,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if alias
        .as_ref()
        .is_some_and(|alias| alias.as_str().is_empty())
    {
        Err(CanonicalSourceClosureSubjectError::new(
            "dependency request contains an empty explicit alias",
        ))
    } else {
        Ok(())
    }
}

fn validate_request_bytes(
    bytes: &[u8],
    maximum_request_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if bytes.len() > maximum_request_bytes {
        Err(CanonicalSourceClosureSubjectError::new(
            "source request violates its byte limit",
        ))
    } else {
        Ok(())
    }
}

fn validate_source_identity(
    source: &ResolvedSourceIdentity,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_package_key(source.key(), maximum_identity_bytes)?;
    if !source
        .resolution()
        .matches_lineage(source.key().source_lineage())
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source resolution disagrees with package lineage",
        ));
    }
    Ok(())
}

fn validate_package_key(
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_identity_string(key.name().as_str(), maximum_identity_bytes)?;
    validate_source_lineage(key.source_lineage(), maximum_identity_bytes)
}

fn validate_source_lineage(
    lineage: &SourceLineage,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    let check = |value: &str| validate_identity_string(value, maximum_identity_bytes);
    match lineage {
        SourceLineage::GitHub(lineage) => {
            check(lineage.owner())?;
            check(lineage.repository())
        }
        SourceLineage::GitLab(lineage) => check(lineage.repository_path()),
        SourceLineage::Git(lineage) => {
            if let Some(user) = lineage.user() {
                check(user)?;
            }
            check(lineage.host())?;
            check(lineage.repository_path())
        }
        SourceLineage::Workspace(lineage) => check(lineage.member_path().as_str()),
        SourceLineage::ExternalLocal(lineage) => {
            check(lineage.canonical_absolute_path().to_str().ok_or_else(|| {
                CanonicalSourceClosureSubjectError::new("external-local lineage path is not UTF-8")
            })?)
        }
    }
}

fn validate_identity_string(
    value: &str,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if value.is_empty() || value.len() > maximum_identity_bytes {
        Err(CanonicalSourceClosureSubjectError::new(
            "source identity violates its byte bounds",
        ))
    } else {
        Ok(())
    }
}

fn encode_subject(
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    dependency_requests: &[CanonicalDependencySourceSelection],
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<Vec<u8>, CanonicalSourceClosureSubjectError> {
    let mut encoder = Encoder::new();
    encoder.fixed(SOURCE_CLOSURE_SUBJECT_MAGIC);
    encoder.u16(SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION);
    encode_root_selection(&mut encoder, root, limits)?;
    encoder.count(packages.len())?;
    for source in packages {
        encode_source_identity(&mut encoder, source, limits.maximum_identity_bytes)?;
    }
    encoder.count(dependency_requests.len())?;
    for request in dependency_requests {
        encode_dependency_selection(&mut encoder, request, limits)?;
    }
    Ok(encoder.finish())
}

fn encode_root_selection(
    encoder: &mut Encoder,
    root: &CanonicalRootSourceSelection,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match &root.request {
        CanonicalRootSourceRequest::Git {
            requested_locator,
            requested_revision,
        } => {
            encoder.byte(0);
            encoder.bytes_bounded(requested_locator.as_bytes(), limits.maximum_request_bytes)?;
            encoder.bytes_bounded(requested_revision.as_bytes(), limits.maximum_request_bytes)?;
        }
        CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => {
            encoder.byte(1);
            encode_source_lineage(
                encoder,
                workspace_root_source,
                limits.maximum_identity_bytes,
            )?;
            encoder.bytes_bounded(
                member_path.as_str().as_bytes(),
                limits.maximum_request_bytes,
            )?;
            encoder.bytes_bounded(requested_workspace_root, limits.maximum_request_bytes)?;
        }
        CanonicalRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => {
            encoder.byte(2);
            encoder.bytes_bounded(requested_root, limits.maximum_request_bytes)?;
            encoder.fixed(&decode_hex_32(&source_context.to_hex())?);
        }
    }
    encode_source_identity(encoder, &root.selected, limits.maximum_identity_bytes)
}

fn decode_root_selection(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<CanonicalRootSourceSelection, CanonicalSourceClosureSubjectError> {
    let request = match decoder.byte()? {
        0 => CanonicalRootSourceRequest::Git {
            requested_locator: decoder.string(limits.maximum_request_bytes)?,
            requested_revision: decoder.string(limits.maximum_request_bytes)?,
        },
        1 => CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source: decode_source_lineage(decoder, limits.maximum_identity_bytes)?,
            member_path: WorkspaceMemberPath::parse(&decoder.string(limits.maximum_request_bytes)?)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new(
                        "invalid workspace member path in root request",
                    )
                })?,
            requested_workspace_root: decoder.bytes(limits.maximum_request_bytes)?.to_vec(),
        },
        2 => CanonicalRootSourceRequest::ExternalLocal {
            requested_root: decoder.bytes(limits.maximum_request_bytes)?.to_vec(),
            source_context: ExternalSourceContext::parse_hex(&encode_hex(&decoder.array_32()?))
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new(
                        "invalid external source context in root request",
                    )
                })?,
        },
        _ => {
            return Err(CanonicalSourceClosureSubjectError::new(
                "invalid root source-request tag",
            ));
        }
    };
    let selected = decode_source_identity(decoder, limits.maximum_identity_bytes)?;
    Ok(CanonicalRootSourceSelection { request, selected })
}

fn encode_dependency_selection(
    encoder: &mut Encoder,
    selection: &CanonicalDependencySourceSelection,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encode_package_key(encoder, &selection.requester, limits.maximum_identity_bytes)?;
    encoder.u32(u32::try_from(selection.dependency_index).map_err(|_| {
        CanonicalSourceClosureSubjectError::new("dependency ordinal exceeds canonical range")
    })?);
    match &selection.request {
        CanonicalDependencySourceRequest::Path {
            explicit_alias,
            location,
        } => {
            encoder.byte(0);
            encode_optional_alias(encoder, explicit_alias, limits.maximum_identity_bytes)?;
            encoder.bytes_bounded(location.as_bytes(), limits.maximum_request_bytes)?;
        }
        CanonicalDependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
        } => {
            encoder.byte(1);
            encode_optional_alias(encoder, explicit_alias, limits.maximum_identity_bytes)?;
            encoder.bytes_bounded(repository.as_bytes(), limits.maximum_request_bytes)?;
            encoder.bytes_bounded(revision.as_bytes(), limits.maximum_request_bytes)?;
        }
    }
    encoder.bytes_bounded(
        selection.alias.as_str().as_bytes(),
        limits.maximum_identity_bytes,
    )?;
    encode_source_identity(encoder, &selection.selected, limits.maximum_identity_bytes)
}

fn decode_dependency_selection(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<CanonicalDependencySourceSelection, CanonicalSourceClosureSubjectError> {
    let requester = decode_package_key(decoder, limits.maximum_identity_bytes)?;
    let dependency_index = usize::try_from(decoder.u32()?).map_err(|_| {
        CanonicalSourceClosureSubjectError::new("dependency ordinal exceeds platform range")
    })?;
    let request = match decoder.byte()? {
        0 => CanonicalDependencySourceRequest::Path {
            explicit_alias: decode_optional_alias(decoder, limits.maximum_identity_bytes)?,
            location: decoder.string(limits.maximum_request_bytes)?,
        },
        1 => CanonicalDependencySourceRequest::Git {
            explicit_alias: decode_optional_alias(decoder, limits.maximum_identity_bytes)?,
            repository: decoder.string(limits.maximum_request_bytes)?,
            revision: decoder.string(limits.maximum_request_bytes)?,
        },
        _ => {
            return Err(CanonicalSourceClosureSubjectError::new(
                "invalid dependency source-request tag",
            ));
        }
    };
    let alias = AliasName::parse(decoder.string(limits.maximum_identity_bytes)?).map_err(|_| {
        CanonicalSourceClosureSubjectError::new("invalid resolved dependency alias")
    })?;
    let selected = decode_source_identity(decoder, limits.maximum_identity_bytes)?;
    Ok(CanonicalDependencySourceSelection {
        requester,
        dependency_index,
        request,
        alias,
        selected,
    })
}

fn encode_optional_alias(
    encoder: &mut Encoder,
    alias: &Option<AliasName>,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match alias {
        None => encoder.byte(0),
        Some(alias) => {
            encoder.byte(1);
            encoder.bytes_bounded(alias.as_str().as_bytes(), maximum_identity_bytes)?;
        }
    }
    Ok(())
}

fn decode_optional_alias(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<Option<AliasName>, CanonicalSourceClosureSubjectError> {
    match decoder.byte()? {
        0 => Ok(None),
        1 => AliasName::parse(decoder.string(maximum_identity_bytes)?)
            .map(Some)
            .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid explicit alias")),
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "invalid explicit-alias option tag",
        )),
    }
}

fn encode_source_identity(
    encoder: &mut Encoder,
    source: &ResolvedSourceIdentity,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encode_package_key(encoder, source.key(), maximum_identity_bytes)?;
    encode_resolution(encoder, source.resolution())
}

fn decode_source_identity(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<ResolvedSourceIdentity, CanonicalSourceClosureSubjectError> {
    ResolvedSourceIdentity::new(
        decode_package_key(decoder, maximum_identity_bytes)?,
        decode_resolution(decoder)?,
    )
    .map_err(|_| {
        CanonicalSourceClosureSubjectError::new(
            "decoded source resolution disagrees with package lineage",
        )
    })
}

fn encode_package_key(
    encoder: &mut Encoder,
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_package_key(key, maximum_identity_bytes)?;
    encoder.bytes_bounded(key.name().as_str().as_bytes(), maximum_identity_bytes)?;
    encode_source_lineage(encoder, key.source_lineage(), maximum_identity_bytes)
}

fn decode_package_key(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<PackageKey, CanonicalSourceClosureSubjectError> {
    let name = PackageName::parse(decoder.string(maximum_identity_bytes)?)
        .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid package name"))?;
    let lineage = decode_source_lineage(decoder, maximum_identity_bytes)?;
    Ok(PackageKey::new(name, lineage))
}

fn encode_source_lineage(
    encoder: &mut Encoder,
    lineage: &SourceLineage,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_source_lineage(lineage, maximum_identity_bytes)?;
    match lineage {
        SourceLineage::GitHub(lineage) => {
            encoder.byte(0);
            encoder.bytes_bounded(lineage.owner().as_bytes(), maximum_identity_bytes)?;
            encoder.bytes_bounded(lineage.repository().as_bytes(), maximum_identity_bytes)?;
        }
        SourceLineage::GitLab(lineage) => {
            encoder.byte(1);
            encoder.bytes_bounded(lineage.repository_path().as_bytes(), maximum_identity_bytes)?;
        }
        SourceLineage::Git(lineage) => {
            encoder.byte(2);
            encoder.byte(match lineage.transport() {
                GitTransport::Https => 0,
                GitTransport::SshUrl => 1,
                GitTransport::ScpLike => 2,
            });
            match lineage.user() {
                None => encoder.byte(0),
                Some(user) => {
                    encoder.byte(1);
                    encoder.bytes_bounded(user.as_bytes(), maximum_identity_bytes)?;
                }
            }
            encoder.bytes_bounded(lineage.host().as_bytes(), maximum_identity_bytes)?;
            match lineage.port() {
                None => encoder.byte(0),
                Some(port) => {
                    encoder.byte(1);
                    encoder.u16(port);
                }
            }
            encoder.bytes_bounded(lineage.repository_path().as_bytes(), maximum_identity_bytes)?;
        }
        SourceLineage::Workspace(lineage) => {
            encoder.byte(3);
            encoder.fixed(&decode_hex_32(&lineage.workspace_identity().to_hex())?);
            encoder.bytes_bounded(
                lineage.member_path().as_str().as_bytes(),
                maximum_identity_bytes,
            )?;
        }
        SourceLineage::ExternalLocal(lineage) => {
            encoder.byte(4);
            encoder.fixed(&decode_hex_32(&lineage.source_context().to_hex())?);
            encoder.bytes_bounded(
                lineage
                    .canonical_absolute_path()
                    .to_str()
                    .ok_or_else(|| {
                        CanonicalSourceClosureSubjectError::new(
                            "external-local lineage path is not UTF-8",
                        )
                    })?
                    .as_bytes(),
                maximum_identity_bytes,
            )?;
        }
    }
    Ok(())
}

fn decode_source_lineage(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<SourceLineage, CanonicalSourceClosureSubjectError> {
    match decoder.byte()? {
        0 => SourceLineage::git(&format!(
            "https://github.com/{}/{}.git",
            decoder.string(maximum_identity_bytes)?,
            decoder.string(maximum_identity_bytes)?
        )),
        1 => SourceLineage::git(&format!(
            "https://gitlab.com/{}.git",
            decoder.string(maximum_identity_bytes)?
        )),
        2 => {
            let transport = match decoder.byte()? {
                0 => GitTransport::Https,
                1 => GitTransport::SshUrl,
                2 => GitTransport::ScpLike,
                _ => {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "invalid Git transport tag",
                    ));
                }
            };
            let user = match decoder.byte()? {
                0 => None,
                1 => Some(decoder.string(maximum_identity_bytes)?),
                _ => {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "invalid Git user option tag",
                    ));
                }
            };
            let host = decoder.string(maximum_identity_bytes)?;
            let port = match decoder.byte()? {
                0 => None,
                1 => Some(decoder.u16()?),
                _ => {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "invalid Git port option tag",
                    ));
                }
            };
            let path = decoder.string(maximum_identity_bytes)?;
            SourceLineage::git(&generic_git_locator(
                transport,
                user.as_deref(),
                &host,
                port,
                &path,
            ))
        }
        3 => {
            let workspace = WorkspaceLineageIdentity::parse_hex(&encode_hex(&decoder.array_32()?))
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid workspace lineage identity")
                })?;
            let member = WorkspaceMemberPath::parse(&decoder.string(maximum_identity_bytes)?)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid workspace member path")
                })?;
            return Ok(SourceLineage::Workspace(WorkspaceMemberLineage::new(
                workspace, member,
            )));
        }
        4 => {
            let context = ExternalSourceContext::parse_hex(&encode_hex(&decoder.array_32()?))
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid external source context")
                })?;
            let path = decoder.string(maximum_identity_bytes)?;
            return ExternalLocalLineage::from_recovered_canonical_path(path, context)
                .map(SourceLineage::ExternalLocal)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid external-local lineage path")
                });
        }
        _ => {
            return Err(CanonicalSourceClosureSubjectError::new(
                "invalid source-lineage tag",
            ));
        }
    }
    .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid source lineage"))
}

fn generic_git_locator(
    transport: GitTransport,
    user: Option<&str>,
    host: &str,
    port: Option<u16>,
    path: &str,
) -> String {
    let user = user.map(|user| format!("{user}@")).unwrap_or_default();
    match transport {
        GitTransport::Https => format!(
            "https://{user}{host}{}/{path}",
            port.map(|port| format!(":{port}")).unwrap_or_default()
        ),
        GitTransport::SshUrl => format!(
            "ssh://{user}{host}{}/{path}",
            port.map(|port| format!(":{port}")).unwrap_or_default()
        ),
        GitTransport::ScpLike => format!("{user}{host}:{path}"),
    }
}

fn encode_resolution(
    encoder: &mut Encoder,
    resolution: &ImmutableSourceResolution,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            encoder.byte(0);
            encoder.bytes_bounded(commit.to_hex().as_bytes(), 64)?;
            encoder.bytes_bounded(tree.to_hex().as_bytes(), 64)?;
            encoder.fixed(&decode_hex_32(&content.to_hex())?);
        }
        ImmutableSourceResolution::Workspace { content } => {
            encoder.byte(1);
            encoder.fixed(&decode_hex_32(&content.to_hex())?);
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            encoder.byte(2);
            encoder.fixed(&decode_hex_32(&content.to_hex())?);
        }
    }
    Ok(())
}

fn decode_resolution(
    decoder: &mut Decoder<'_>,
) -> Result<ImmutableSourceResolution, CanonicalSourceClosureSubjectError> {
    let content = |decoder: &mut Decoder<'_>| {
        SourceContentDigest::parse_hex(&encode_hex(&decoder.array_32()?))
            .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid source content digest"))
    };
    match decoder.byte()? {
        0 => ImmutableSourceResolution::git(
            GitCommitId::parse_hex(&decoder.string(64)?)
                .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid Git commit ID"))?,
            GitTreeId::parse_hex(&decoder.string(64)?)
                .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid Git tree ID"))?,
            content(decoder)?,
        )
        .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid Git source resolution")),
        1 => Ok(ImmutableSourceResolution::workspace(content(decoder)?)),
        2 => Ok(ImmutableSourceResolution::external_local(content(decoder)?)),
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "invalid immutable-resolution tag",
        )),
    }
}

fn fingerprint(bytes: &[u8]) -> CanonicalSourceClosureSubjectFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    CanonicalSourceClosureSubjectFingerprint(hasher.finalize().into())
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], CanonicalSourceClosureSubjectError> {
    let bytes = decode_hex(value).ok_or_else(|| {
        CanonicalSourceClosureSubjectError::new("invalid 32-byte hexadecimal value")
    })?;
    bytes
        .try_into()
        .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid 32-byte hexadecimal value"))
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|digits| {
            let high = hex_value(digits[0])?;
            let low = hex_value(digits[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn fixed(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.fixed(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.fixed(&value.to_le_bytes());
    }

    fn count(&mut self, value: usize) -> Result<(), CanonicalSourceClosureSubjectError> {
        self.u32(u32::try_from(value).map_err(|_| {
            CanonicalSourceClosureSubjectError::new("canonical sequence count exceeds u32")
        })?);
        Ok(())
    }

    fn bytes_bounded(
        &mut self,
        value: &[u8],
        maximum_bytes: usize,
    ) -> Result<(), CanonicalSourceClosureSubjectError> {
        if value.len() > maximum_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "canonical field exceeds its byte limit",
            ));
        }
        self.count(value.len())?;
        self.fixed(value);
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CanonicalSourceClosureSubjectError> {
        let end = self.cursor.checked_add(count).ok_or_else(|| {
            CanonicalSourceClosureSubjectError::new("source-closure subject offset overflow")
        })?;
        let bytes = self.bytes.get(self.cursor..end).ok_or_else(|| {
            CanonicalSourceClosureSubjectError::new("truncated source-closure subject")
        })?;
        self.cursor = end;
        Ok(bytes)
    }

    fn expect_fixed(&mut self, expected: &[u8]) -> Result<(), CanonicalSourceClosureSubjectError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(CanonicalSourceClosureSubjectError::new(
                "invalid source-closure subject header",
            ))
        }
    }

    fn byte(&mut self) -> Result<u8, CanonicalSourceClosureSubjectError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, CanonicalSourceClosureSubjectError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, CanonicalSourceClosureSubjectError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn count(&mut self, maximum: usize) -> Result<usize, CanonicalSourceClosureSubjectError> {
        let count = usize::try_from(self.u32()?).map_err(|_| {
            CanonicalSourceClosureSubjectError::new("canonical count exceeds platform range")
        })?;
        if count > maximum {
            return Err(CanonicalSourceClosureSubjectError::new(
                "canonical count exceeds its resource limit",
            ));
        }
        Ok(count)
    }

    fn bytes(&mut self, maximum: usize) -> Result<&'a [u8], CanonicalSourceClosureSubjectError> {
        let count = self.count(maximum)?;
        self.take(count)
    }

    fn string(&mut self, maximum: usize) -> Result<String, CanonicalSourceClosureSubjectError> {
        String::from_utf8(self.bytes(maximum)?.to_vec())
            .map_err(|_| CanonicalSourceClosureSubjectError::new("canonical string is not UTF-8"))
    }

    fn array_32(&mut self) -> Result<[u8; 32], CanonicalSourceClosureSubjectError> {
        Ok(self.take(32)?.try_into().unwrap())
    }

    fn finish(self) -> Result<(), CanonicalSourceClosureSubjectError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject has trailing bytes",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_source(name: &str, repository: &str, marker: u8) -> ResolvedSourceIdentity {
        let key = PackageKey::new(
            PackageName::parse(name).unwrap(),
            SourceLineage::git(&format!("https://github.com/CathedralOS/{repository}.git"))
                .unwrap(),
        );
        let digit = char::from_digit(u32::from(marker % 10), 16).unwrap();
        let next = char::from_digit(u32::from((marker + 1) % 10), 16).unwrap();
        let resolution = ImmutableSourceResolution::git(
            GitCommitId::parse_hex(&digit.to_string().repeat(40)).unwrap(),
            GitTreeId::parse_hex(&next.to_string().repeat(40)).unwrap(),
            SourceContentDigest::derive(&[marker]),
        )
        .unwrap();
        ResolvedSourceIdentity::new(key, resolution).unwrap()
    }

    fn root_git_selection(
        locator: &str,
        selected: &ResolvedSourceIdentity,
    ) -> CanonicalRootSourceSelection {
        CanonicalRootSourceSelection {
            request: CanonicalRootSourceRequest::Git {
                requested_locator: locator.to_owned(),
                requested_revision: "main".to_owned(),
            },
            selected: selected.clone(),
        }
    }

    #[test]
    fn exact_git_request_spelling_changes_subject_without_changing_selection() {
        let selected = git_source("codec", "codec", 1);
        let https = CanonicalSourceClosureSubject::finish(
            root_git_selection("https://github.com/CathedralOS/codec.git", &selected),
            vec![selected.clone()],
            Vec::new(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap();
        let ssh = CanonicalSourceClosureSubject::finish(
            root_git_selection("git@github.com:CathedralOS/codec.git", &selected),
            vec![selected],
            Vec::new(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap();

        assert_eq!(https.root.selected, ssh.root.selected);
        assert_ne!(https.canonical_bytes, ssh.canonical_bytes);
        assert_ne!(https.fingerprint, ssh.fingerprint);
        assert_eq!(
            CanonicalSourceClosureSubject::recover(
                https.canonical_bytes(),
                CanonicalSourceClosureSubjectLimits::default(),
            )
            .unwrap(),
            https
        );
    }

    #[test]
    fn request_and_edge_disagreement_reject_before_encoding() {
        let root = git_source("root", "root", 1);
        let child = git_source("child", "child", 2);
        let request = CanonicalDependencySourceSelection {
            requester: root.key().clone(),
            dependency_index: 0,
            request: CanonicalDependencySourceRequest::Git {
                explicit_alias: None,
                repository: "https://github.com/CathedralOS/child.git".to_owned(),
                revision: "main".to_owned(),
            },
            alias: AliasName::parse("wrong_alias").unwrap(),
            selected: child.clone(),
        };
        let error = CanonicalSourceClosureSubject::finish(
            root_git_selection("https://github.com/CathedralOS/root.git", &root),
            vec![child, root],
            vec![request],
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.message(),
            "dependency request alias disagrees with its authored selection"
        );
    }

    #[test]
    fn missing_ordinals_and_open_graphs_reject() {
        let root = git_source("root", "root", 1);
        let child = git_source("child", "child", 2);
        let request = |dependency_index| CanonicalDependencySourceSelection {
            requester: root.key().clone(),
            dependency_index,
            request: CanonicalDependencySourceRequest::Git {
                explicit_alias: None,
                repository: "https://github.com/CathedralOS/child.git".to_owned(),
                revision: "main".to_owned(),
            },
            alias: child.key().name().default_alias(),
            selected: child.clone(),
        };
        let error = CanonicalSourceClosureSubject::finish(
            root_git_selection("https://github.com/CathedralOS/root.git", &root),
            vec![child.clone(), root.clone()],
            vec![request(1)],
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.message(),
            "dependency request ordinals do not begin at zero"
        );

        let error = CanonicalSourceClosureSubject::finish(
            root_git_selection("https://github.com/CathedralOS/root.git", &root),
            vec![root.clone()],
            vec![request(0)],
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.message(),
            "dependency request selection is absent or resolution-mismatched"
        );
    }

    #[test]
    fn recovery_rejects_unknown_version_trailing_bytes_and_tight_limits() {
        let selected = git_source("codec", "codec", 1);
        let subject = CanonicalSourceClosureSubject::finish(
            root_git_selection("https://github.com/CathedralOS/codec.git", &selected),
            vec![selected],
            Vec::new(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap();

        let mut unknown_version = subject.canonical_bytes.clone();
        let version_offset = SOURCE_CLOSURE_SUBJECT_MAGIC.len();
        unknown_version[version_offset..version_offset + 2].copy_from_slice(&2_u16.to_le_bytes());
        assert!(
            CanonicalSourceClosureSubject::recover(
                &unknown_version,
                CanonicalSourceClosureSubjectLimits::default()
            )
            .is_err()
        );

        let mut trailing = subject.canonical_bytes.clone();
        trailing.push(0);
        assert!(
            CanonicalSourceClosureSubject::recover(
                &trailing,
                CanonicalSourceClosureSubjectLimits::default()
            )
            .is_err()
        );

        let limits = CanonicalSourceClosureSubjectLimits {
            maximum_record_bytes: subject.canonical_bytes.len() - 1,
            ..CanonicalSourceClosureSubjectLimits::default()
        };
        assert!(CanonicalSourceClosureSubject::recover(subject.canonical_bytes(), limits).is_err());
    }

    #[test]
    fn noncanonical_unreachable_and_cyclic_package_state_rejects() {
        let root = git_source("root", "root", 1);
        let child = git_source("child", "child", 2);
        let root_selection = root_git_selection("https://github.com/CathedralOS/root.git", &root);

        let error = CanonicalSourceClosureSubject::finish(
            root_selection.clone(),
            vec![root.clone(), child.clone()],
            Vec::new(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.message(),
            "source-closure packages are not in strict canonical order"
        );

        let error = CanonicalSourceClosureSubject::finish(
            root_selection.clone(),
            vec![child.clone(), root.clone()],
            Vec::new(),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.message(),
            "source-closure subject does not form one closed reachable acyclic graph"
        );

        let request = |requester: &ResolvedSourceIdentity,
                       selected: &ResolvedSourceIdentity,
                       repository: &str| CanonicalDependencySourceSelection {
            requester: requester.key().clone(),
            dependency_index: 0,
            request: CanonicalDependencySourceRequest::Git {
                explicit_alias: None,
                repository: repository.to_owned(),
                revision: "main".to_owned(),
            },
            alias: selected.key().name().default_alias(),
            selected: selected.clone(),
        };
        let error = CanonicalSourceClosureSubject::finish(
            root_selection,
            vec![child.clone(), root.clone()],
            vec![
                request(&child, &root, "https://github.com/CathedralOS/root.git"),
                request(&root, &child, "https://github.com/CathedralOS/child.git"),
            ],
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.message(),
            "source-closure subject does not form one closed reachable acyclic graph"
        );
    }
}
