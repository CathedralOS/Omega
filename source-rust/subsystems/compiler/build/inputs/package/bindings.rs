// Caller-supplied binding carriers for one package compilation:
// `PackageSourceBinding` binds a package identity, canonical name, physical
// source root, and optional captured source tree or `Build::Source` metadata
// index; `PackageDependencyBinding` binds a requester-local alias in `Product`
// or `Build` scope; and `IndependentComponentDescription` carries unverified
// component description bytes alongside the expected `TerminalPsiIdentity` for
// build settlement verification.

use super::canonical_source_root;
use crate::source::source_snapshot;
use build_declarations::DependencyPurpose;
use filesystem_authority::CanonicalFilesystemMetadataIndex;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceBinding {
    pub(super) build_entries: super::PackageBuildEntries,
    pub(super) identity: PackageKeyIdentity,
    pub(super) canonical_name: String,
    pub(super) resolution: Option<[u8; 32]>,
    pub(super) source_root: PathBuf,
    pub(super) preparing_source: Option<crate::CapturedLocalSource>,
    pub(super) canonical_source_metadata: Option<CanonicalFilesystemMetadataIndex>,
    pub(super) captured_source: Option<Arc<source_capture::CapturedSourceTree>>,
    pub(super) captured_build_source: Option<build_output::CapturedBuildSourceInput>,
}

impl PackageSourceBinding {
    pub fn new(
        identity: PackageKeyIdentity,
        canonical_name: impl Into<String>,
        source_root: PathBuf,
    ) -> Self {
        Self {
            build_entries: Default::default(),
            identity,
            canonical_name: canonical_name.into(),
            resolution: None,
            source_root,
            preparing_source: None,
            canonical_source_metadata: None,
            captured_source: None,
            captured_build_source: None,
        }
    }

    /// A returned directory changes what the compiler reads. The original package
    /// still determines whether two acquisitions overlapped on the host.
    pub fn with_preparing_source(mut self, source: Option<crate::CapturedLocalSource>) -> Self {
        self.preparing_source = source;
        self
    }

    /// Reuse the root's prepared compiler input only for the same acquisition
    /// and the same directory returned by the Build.
    pub fn can_reuse_for(
        &self,
        requested: &Self,
        entries: &super::PackageBuildEntries,
        selected: Option<&Arc<source_capture::CapturedSourceTree>>,
    ) -> bool {
        self.identity == requested.identity
            && self.canonical_name == requested.canonical_name
            && self.resolution == requested.resolution
            && self.source_root == requested.source_root
            && self.preparing_source == requested.preparing_source
            && &self.build_entries == entries
            && self.captured_source.as_ref() == selected
    }

    pub fn with_build_entries(
        mut self,
        entries: super::PackageBuildEntries,
    ) -> Result<Self, String> {
        let tree = self
            .captured_source
            .as_deref()
            .ok_or("entered Build files require a captured package")?;
        entries.require_capture(tree)?;
        self.build_entries = entries;
        Ok(self)
    }

    pub fn with_resolution(
        mut self,
        resolution: &source_acquisition::ImmutableSourceResolution,
    ) -> Self {
        self.resolution = Some(resolution.identity_digest());
        self
    }

    pub fn with_captured_source(
        mut self,
        tree: Arc<source_capture::CapturedSourceTree>,
    ) -> Result<Self, String> {
        self.build_entries.require_capture(&tree)?;
        let input = source_snapshot::retained_source_input(&tree)?;
        self.canonical_source_metadata = None;
        self.captured_source = Some(tree);
        self.captured_build_source = Some(input);
        Ok(self)
    }

    pub fn with_canonical_source_metadata(mut self) -> Result<Self, String> {
        if self.captured_source.is_some() {
            return Ok(self);
        }
        let canonical_root = canonical_source_root(&self.source_root)?;
        self.canonical_source_metadata = Some(source_snapshot::capture(&canonical_root)?);
        Ok(self)
    }

    pub const fn identity(&self) -> PackageKeyIdentity {
        self.identity
    }

    pub fn canonical_name(&self) -> &str {
        &self.canonical_name
    }

    pub fn build_entries(&self) -> &super::PackageBuildEntries {
        &self.build_entries
    }

    pub fn source_root(&self) -> &Path {
        &self.source_root
    }

    pub fn canonical_source_metadata(&self) -> Option<&CanonicalFilesystemMetadataIndex> {
        self.canonical_source_metadata.as_ref().or_else(|| {
            self.captured_build_source
                .as_ref()
                .map(|input| input.canonical_source_metadata())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyBinding {
    pub(super) max_reach: semantic_vocabulary::dependency_reach::DependencyReach,
    pub(super) requester: PackageKeyIdentity,
    pub(super) alias: String,
    pub(super) target: PackageKeyIdentity,
    pub(super) purpose: DependencyPurpose,
}

impl PackageDependencyBinding {
    pub fn new(
        requester: PackageKeyIdentity,
        alias: impl Into<String>,
        target: PackageKeyIdentity,
    ) -> Self {
        Self::for_purpose(requester, alias, target, DependencyPurpose::Product)
    }

    pub fn for_purpose(
        requester: PackageKeyIdentity,
        alias: impl Into<String>,
        target: PackageKeyIdentity,
        purpose: DependencyPurpose,
    ) -> Self {
        Self {
            requester,
            alias: alias.into(),
            max_reach: Default::default(),
            target,
            purpose,
        }
    }

    pub fn with_max_reach(
        mut self,
        max_reach: semantic_vocabulary::dependency_reach::DependencyReach,
    ) -> Self {
        self.max_reach = max_reach;
        self
    }

    pub fn max_reach(&self) -> &semantic_vocabulary::dependency_reach::DependencyReach {
        &self.max_reach
    }

    pub const fn requester(&self) -> PackageKeyIdentity {
        self.requester
    }

    pub fn alias(&self) -> &str {
        &self.alias
    }

    pub const fn target(&self) -> PackageKeyIdentity {
        self.target
    }

    pub const fn purpose(&self) -> DependencyPurpose {
        self.purpose
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndependentComponentDescription {
    package: PackageKeyIdentity,
    expected_subject: TerminalPsiIdentity,
    /// Independent selections of this package's checked Product for the same target.
    independent_suppliers: BTreeSet<PackageKeyIdentity>,
    description: Arc<[u8]>,
}

impl IndependentComponentDescription {
    pub fn new(
        package: PackageKeyIdentity,
        expected_subject: TerminalPsiIdentity,
        independent_suppliers: BTreeSet<PackageKeyIdentity>,
        description: Vec<u8>,
    ) -> Self {
        Self {
            package,
            expected_subject,
            independent_suppliers,
            description: Arc::from(description),
        }
    }

    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn expected_subject(&self) -> TerminalPsiIdentity {
        self.expected_subject
    }

    pub fn independent_suppliers(&self) -> &BTreeSet<PackageKeyIdentity> {
        &self.independent_suppliers
    }

    pub fn description(&self) -> &[u8] {
        &self.description
    }
}
