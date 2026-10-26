//! Carriers a caller binds into one package compilation: package source
//! roots, requester-local dependency edges, exact build-input occurrence
//! coordinates, and independently composed component descriptions.

use super::canonical_source_root;
use crate::source_snapshot;
use build_declarations::DependencyPurpose;
use filesystem_authority::CanonicalFilesystemMetadataIndex;
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use terminal_psi::TerminalPsiIdentity;

/// One stable package identity, its canonical declared name, and the canonical
/// source root from which this compilation may load it. The name is validated
/// diagnostic metadata and the path is custody/routing data; neither replaces
/// the opaque identity in semantic comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceBinding {
    pub(super) identity: PackageKeyIdentity,
    pub(super) canonical_name: String,
    pub(super) source_root: PathBuf,
    pub(super) canonical_source_metadata: Option<CanonicalFilesystemMetadataIndex>,
    pub(super) captured_source: Option<Arc<source::CapturedSourceTree>>,
    pub(super) captured_build_source: Option<build_output::CapturedBuildSourceInput>,
}

impl PackageSourceBinding {
    pub fn new(
        identity: PackageKeyIdentity,
        canonical_name: impl Into<String>,
        source_root: PathBuf,
    ) -> Self {
        Self {
            identity,
            canonical_name: canonical_name.into(),
            source_root,
            canonical_source_metadata: None,
            captured_source: None,
            captured_build_source: None,
        }
    }

    /// Carry acquisition's immutable namespace and derive Build commitments
    /// from its retained bytes, never from a later version of the checkout.
    pub fn with_captured_source(
        mut self,
        tree: Arc<source::CapturedSourceTree>,
    ) -> Result<Self, String> {
        let input = source_snapshot::retained_source_input(&tree)?;
        self.canonical_source_metadata = None;
        self.captured_source = Some(tree);
        self.captured_build_source = Some(input);
        Ok(self)
    }

    /// Capture compiler-owned canonical build-visible metadata from this exact
    /// physical root. Callers cannot supply rows or a content commitment.
    pub fn with_canonical_source_metadata(mut self) -> Result<Self, String> {
        if self.captured_source.is_some() {
            return Ok(self);
        }
        let canonical_root = canonical_source_root(&self.source_root)?;
        self.canonical_source_metadata = Some(source_snapshot::capture(&canonical_root)?);
        Ok(self)
    }

    /// The same capture routed through one persistent checked-source cache:
    /// an unchanged root replays its retained index, and any drift rebuilds
    /// through the full hashing capture. The bound index is identical either
    /// way.
    pub fn with_cached_canonical_source_metadata(
        mut self,
        cache: &crate::checked_source_cache::CheckedSourceCache,
    ) -> Result<Self, String> {
        if self.captured_source.is_some() {
            return Ok(self);
        }
        self.canonical_source_metadata = Some(
            cache
                .capture(&self.source_root)?
                .canonical_source_metadata()
                .clone(),
        );
        Ok(self)
    }

    pub const fn identity(&self) -> PackageKeyIdentity {
        self.identity
    }

    pub fn canonical_name(&self) -> &str {
        &self.canonical_name
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

/// One requester-local alias selected by the reconciled package graph for one
/// authorized dependency scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyBinding {
    pub(super) requester: PackageKeyIdentity,
    pub(super) alias: String,
    pub(super) target: PackageKeyIdentity,
    pub(super) purpose: DependencyPurpose,
}

impl PackageDependencyBinding {
    /// One product-scope edge. Durable `PackageDependencyClosure` rows are
    /// product-only: build edges are compilation-local nameability and never
    /// join a persisted closure.
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
            target,
            purpose,
        }
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

/// One dependency's canonical component description, attached to the exact
/// target compilation that selected that dependency's providers with
/// `CompositionMode::Independent`.
///
/// The bytes are the description the dependency's own compilation published;
/// `expected_subject` is the Terminal Psi identity that compilation observed
/// for the component module, not a field read out of the description. This
/// carrier is trusted for nothing: build settlement verifies the bytes under
/// the build's admission profile against the expected subject before any
/// selected plan may join the component, so a substituted, corrupt, or
/// partial description rejects there instead of being carried as evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndependentComponentDescription {
    package: PackageKeyIdentity,
    expected_subject: TerminalPsiIdentity,
    description: Arc<[u8]>,
}

impl IndependentComponentDescription {
    pub fn new(
        package: PackageKeyIdentity,
        expected_subject: TerminalPsiIdentity,
        description: Vec<u8>,
    ) -> Self {
        Self {
            package,
            expected_subject,
            description: Arc::from(description),
        }
    }

    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn expected_subject(&self) -> TerminalPsiIdentity {
        self.expected_subject
    }

    /// The exact canonical description bytes to verify. Their identity is
    /// established by the verifier, never by this carrier.
    pub fn description(&self) -> &[u8] {
        &self.description
    }
}
