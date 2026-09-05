use super::PackagePolicySourceReplacement;
use crate::declarations::PackageKey;
use crate::resolution::graph::CanonicalSourceClosureSubjectFingerprint;
use crate::review::compare::model::ReviewOnlyRootRoleChange;
use package_evidence::record::{PackagePolicyRow, PackagePolicyRowKind};
use package_source::ImmutableSourceResolution;
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyChangeKind {
    Added,
    Removed,
    Changed,
}

/// A versioned identity of exact comparison context, never review certification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackagePolicyChangeFingerprint(pub(super) [u8; 32]);
impl PackagePolicyChangeFingerprint {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRowChange {
    pub(super) baseline: Option<PackagePolicyRow>,
    pub(super) candidate: Option<PackagePolicyRow>,
    pub(super) change: PackagePolicyChangeKind,
    pub(super) requires_decision: bool,
    pub(super) audit_recommended: bool,
    pub(super) fingerprint: PackagePolicyChangeFingerprint,
}
impl PackagePolicyRowChange {
    fn row(&self) -> &PackagePolicyRow {
        self.candidate
            .as_ref()
            .or(self.baseline.as_ref())
            .expect("delta contains one row")
    }
    pub fn kind(&self) -> PackagePolicyRowKind {
        self.row().kind()
    }
    pub fn key_bytes(&self) -> &[u8] {
        self.row().key_bytes()
    }
    pub const fn change(&self) -> PackagePolicyChangeKind {
        self.change
    }
    pub const fn baseline(&self) -> Option<&PackagePolicyRow> {
        self.baseline.as_ref()
    }
    pub const fn candidate(&self) -> Option<&PackagePolicyRow> {
        self.candidate.as_ref()
    }
    pub const fn requires_decision(&self) -> bool {
        self.requires_decision
    }
    pub const fn audit_recommended(&self) -> bool {
        self.audit_recommended
    }
    pub const fn fingerprint(&self) -> PackagePolicyChangeFingerprint {
        self.fingerprint
    }
}

/// Source-qualified path from one side's root, without old filesystem custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyDependencyPath {
    pub(super) root: PackageKeyIdentity,
    pub(super) steps: Vec<PackagePolicyDependencyPathStep>,
}
impl PackagePolicyDependencyPath {
    pub const fn root(&self) -> PackageKeyIdentity {
        self.root
    }
    pub fn steps(&self) -> &[PackagePolicyDependencyPathStep] {
        &self.steps
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyDependencyPathStep {
    pub(super) requester: PackageKeyIdentity,
    pub(super) dependency_index: usize,
    pub(super) alias: String,
    pub(super) target: PackageKeyIdentity,
}
impl PackagePolicyDependencyPathStep {
    pub const fn requester(&self) -> PackageKeyIdentity {
        self.requester
    }
    pub const fn dependency_index(&self) -> usize {
        self.dependency_index
    }
    pub fn alias(&self) -> &str {
        &self.alias
    }
    pub const fn target(&self) -> PackageKeyIdentity {
        self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyPackageChange {
    pub(super) key: PackageKey,
    pub(super) baseline_resolution: Option<ImmutableSourceResolution>,
    pub(super) candidate_resolution: Option<ImmutableSourceResolution>,
    pub(super) baseline_path: Option<PackagePolicyDependencyPath>,
    pub(super) candidate_path: Option<PackagePolicyDependencyPath>,
    pub(super) source_changed: bool,
    pub(super) source_association_changed: bool,
    pub(super) audit_recommended: bool,
    pub(super) rows: Vec<PackagePolicyRowChange>,
    pub(super) fingerprint: PackagePolicyChangeFingerprint,
}
impl PackagePolicyPackageChange {
    pub const fn key(&self) -> &PackageKey {
        &self.key
    }
    pub const fn baseline_resolution(&self) -> Option<&ImmutableSourceResolution> {
        self.baseline_resolution.as_ref()
    }
    pub const fn candidate_resolution(&self) -> Option<&ImmutableSourceResolution> {
        self.candidate_resolution.as_ref()
    }
    pub const fn baseline_path(&self) -> Option<&PackagePolicyDependencyPath> {
        self.baseline_path.as_ref()
    }
    pub const fn candidate_path(&self) -> Option<&PackagePolicyDependencyPath> {
        self.candidate_path.as_ref()
    }
    pub const fn source_changed(&self) -> bool {
        self.source_changed
    }
    pub const fn source_association_changed(&self) -> bool {
        self.source_association_changed
    }
    pub const fn audit_recommended(&self) -> bool {
        self.audit_recommended
    }
    pub fn rows(&self) -> &[PackagePolicyRowChange] {
        &self.rows
    }
    pub fn requires_decision(&self) -> bool {
        self.rows
            .iter()
            .any(PackagePolicyRowChange::requires_decision)
    }
    pub const fn fingerprint(&self) -> PackagePolicyChangeFingerprint {
        self.fingerprint
    }
}

/// Complete package-key union. Unchanged candidates remain visible for audit.
/// Historical resolutions cannot consume this type through a legacy adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyChangeSet {
    pub(super) baseline_source_subject: Option<CanonicalSourceClosureSubjectFingerprint>,
    pub(super) candidate_source_subject: CanonicalSourceClosureSubjectFingerprint,
    pub(super) fingerprint: PackagePolicyChangeFingerprint,
    pub(super) root_changed: bool,
    pub(super) source_subject_changed: bool,
    pub(super) root_role_change: Option<ReviewOnlyRootRoleChange>,
    pub(super) source_replacements: Vec<PackagePolicySourceReplacement>,
    pub(super) packages: Vec<PackagePolicyPackageChange>,
}
impl PackagePolicyChangeSet {
    pub const fn baseline_source_subject(
        &self,
    ) -> Option<&CanonicalSourceClosureSubjectFingerprint> {
        self.baseline_source_subject.as_ref()
    }
    pub const fn candidate_source_subject(&self) -> &CanonicalSourceClosureSubjectFingerprint {
        &self.candidate_source_subject
    }
    pub const fn fingerprint(&self) -> PackagePolicyChangeFingerprint {
        self.fingerprint
    }
    pub const fn root_changed(&self) -> bool {
        self.root_changed
    }
    pub const fn source_subject_changed(&self) -> bool {
        self.source_subject_changed
    }
    pub const fn root_role_change(&self) -> Option<&ReviewOnlyRootRoleChange> {
        self.root_role_change.as_ref()
    }
    pub fn packages(&self) -> &[PackagePolicyPackageChange] {
        &self.packages
    }
    pub fn requires_decision(&self) -> bool {
        self.root_role_change.is_some()
            || !self.source_replacements.is_empty()
            || self
                .packages
                .iter()
                .any(PackagePolicyPackageChange::requires_decision)
    }
    pub fn audit_recommended(&self) -> bool {
        self.source_subject_changed
            || self
                .packages
                .iter()
                .any(PackagePolicyPackageChange::audit_recommended)
    }

    pub fn source_replacements(&self) -> &[PackagePolicySourceReplacement] {
        &self.source_replacements
    }
}
