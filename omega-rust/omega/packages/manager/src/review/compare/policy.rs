//! Complete normalized policy deltas; neither decisions nor publication.

mod assembly;
mod error;
mod fingerprints;
mod limits;
mod merge;
mod paths;
mod projection;
mod replacements;

pub use error::PackagePolicyChangeError;
pub use limits::PackagePolicyChangeLimits;
pub use replacements::{PackagePolicyReplacementSite, PackagePolicySourceReplacement};

use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::declarations::dependencies::DependencyPurpose;
use crate::lock::PackageLockTarget;
use crate::lock::{PackageAcceptanceRow, PackageCheckedContext};
use crate::resolution::graph::CanonicalSourceClosureSubjectFingerprint;
use crate::resolution::graph::{CanonicalSourceClosureSubject, ExactTargetPackageSourceClosure};
use crate::review::timings;
use crate::review::{
    CompilerIssuedPackageReviewSet, ReviewOnlyRootRoleChange, ReviewOnlyRootRoleContract,
};
use limits::Budget;
use package_evidence::record::PackagePolicyRowKind;
use package_source::ImmutableSourceResolution;
use semantic_vocabulary::PackageKeyIdentity;
use sha2::Digest;

/// Compare one complete fresh candidate against retained policy or an explicit
/// empty initial baseline. Old content is never reacquired or compiler-replayed.
/// Removed packages remain present with no candidate resolution or path.
///
/// Candidate verification and obligation discharge remain independent. This
/// function does not reinterpret historical decisions or authorize a transaction.
pub fn compare_package_policy_changes(
    accepted: Option<&PackageLockTarget>,
    candidate: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ExactTargetPackageSourceClosure<'_>,
    limits: PackagePolicyChangeLimits,
) -> Result<PackagePolicyChangeSet, PackagePolicyChangeError> {
    let _stage = timings::stage("policy_change_comparison");
    let mut budget = Budget::new(limits);
    if accepted.is_some_and(|old| old.target() != candidate_sources.target_profile()) {
        return Err(PackagePolicyChangeError::TargetMismatch);
    }
    let source =
        CanonicalSourceClosureSubject::from_resolved(candidate_sources, budget.subject_limits())
            .map_err(PackagePolicyChangeError::SourceSubject)?;
    let reviews = projection::candidate(candidate, candidate_sources, &source, &mut budget)?;
    budget.context(source.canonical_bytes().len())?;
    if let Some(old) = accepted {
        budget.context(old.source().canonical_bytes().len())?;
    }
    let mut context = fingerprints::context(accepted, &source);
    let mut packages = assembly::packages(accepted, &source, &reviews, &mut budget, &mut context)?;
    let fingerprint = PackagePolicyChangeFingerprint(context.finalize().into());
    for package in &mut packages {
        fingerprints::finish_package(fingerprint, package);
    }
    let root_changed = accepted.is_some_and(|old| {
        old.source().root().selected().key() != source.root().selected().key()
            || old.source().root_role() != source.root_role()
    });
    let source_subject_changed =
        accepted.is_some_and(|old| !old.source().same_source_graph(&source));
    let root_role_change = accepted
        .map(|old| role_change(old.source(), &source, &mut budget))
        .transpose()?
        .flatten();
    let source_replacements = replacements::compare(
        accepted.map(PackageLockTarget::source),
        &source,
        fingerprint,
        &mut budget,
    )?;
    Ok(PackagePolicyChangeSet {
        baseline_source_subject: accepted.map(|old| old.source().fingerprint().clone()),
        candidate_source_subject: source.fingerprint().clone(),
        fingerprint,
        root_changed,
        source_subject_changed,
        root_role_change,
        source_replacements,
        packages,
    })
}

fn role_change(
    old: &CanonicalSourceClosureSubject,
    new: &CanonicalSourceClosureSubject,
    budget: &mut Budget,
) -> Result<Option<ReviewOnlyRootRoleChange>, PackagePolicyChangeError> {
    if old.root().selected().key() != new.root().selected().key() {
        return Ok(None);
    }
    let broken_contract = match (old.root_role(), new.root_role()) {
        (BuildDeclarationKind::Package, BuildDeclarationKind::Application) => {
            ReviewOnlyRootRoleContract::DependencyCompatibility
        }
        (BuildDeclarationKind::Application, BuildDeclarationKind::Package) => {
            ReviewOnlyRootRoleContract::ApplicationActivation
        }
        _ => return Ok(None),
    };
    budget.key(old.root().selected().key())?;
    Ok(Some(ReviewOnlyRootRoleChange {
        root: old.root().selected().key().clone(),
        baseline_role: old.root_role(),
        candidate_role: new.root_role(),
        broken_contract,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyChangeKind {
    Added,
    Removed,
    Changed,
}

/// A versioned identity of exact comparison context, never review certification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackagePolicyChangeFingerprint([u8; 32]);
impl PackagePolicyChangeFingerprint {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRowChange {
    baseline_context: Option<PackageCheckedContext>,
    candidate_context: Option<PackageCheckedContext>,
    baseline: Option<PackageAcceptanceRow>,
    candidate: Option<PackageAcceptanceRow>,
    change: PackagePolicyChangeKind,
    requires_decision: bool,
    audit_recommended: bool,
    fingerprint: PackagePolicyChangeFingerprint,
}
impl PackagePolicyRowChange {
    pub const fn baseline_context(&self) -> Option<PackageCheckedContext> {
        self.baseline_context
    }
    pub const fn candidate_context(&self) -> Option<PackageCheckedContext> {
        self.candidate_context
    }
    fn row(&self) -> &PackageAcceptanceRow {
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
    pub const fn baseline(&self) -> Option<&PackageAcceptanceRow> {
        self.baseline.as_ref()
    }
    pub const fn candidate(&self) -> Option<&PackageAcceptanceRow> {
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
    root: PackageKeyIdentity,
    steps: Vec<PackagePolicyDependencyPathStep>,
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
    requester: PackageKeyIdentity,
    purpose: DependencyPurpose,
    dependency_index: usize,
    alias: String,
    target: PackageKeyIdentity,
}
impl PackagePolicyDependencyPathStep {
    pub const fn requester(&self) -> PackageKeyIdentity {
        self.requester
    }
    pub const fn purpose(&self) -> DependencyPurpose {
        self.purpose
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
    key: PackageKey,
    baseline_resolution: Option<ImmutableSourceResolution>,
    candidate_resolution: Option<ImmutableSourceResolution>,
    baseline_path: Option<PackagePolicyDependencyPath>,
    candidate_path: Option<PackagePolicyDependencyPath>,
    /// The candidate's normalized restricted build-host requests — what this
    /// package's build machine asks of the host before it may run. They are
    /// admission intent surfaced beside, never inside, this package's
    /// product-authority rows; accepting the rows does not accept host reach.
    restricted_build_requests: Vec<build_evaluation::RestrictedBuildRequest>,
    /// The occurrences the accepted baseline consented to, joined from the
    /// source-graph roster; empty when the package has no baseline.
    baseline_occurrence_contexts: Vec<PackageCheckedContext>,
    /// The occurrences the candidate review answers for, joined from the
    /// candidate roster; empty when the package left the closure.
    candidate_occurrence_contexts: Vec<PackageCheckedContext>,
    source_changed: bool,
    source_association_changed: bool,
    audit_recommended: bool,
    rows: Vec<PackagePolicyRowChange>,
    fingerprint: PackagePolicyChangeFingerprint,
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
    /// The candidate's restricted build-host requests in issue order
    /// (wiki/spec/packages/acceptance.md#restricted-build-acceptance). Empty
    /// when the package has no build machine or its build asks nothing of
    /// the host.
    pub fn restricted_build_requests(&self) -> &[build_evaluation::RestrictedBuildRequest] {
        &self.restricted_build_requests
    }
    /// Exact retained checked occurrences; empty when the package has no baseline.
    pub fn baseline_occurrence_contexts(&self) -> &[PackageCheckedContext] {
        &self.baseline_occurrence_contexts
    }
    /// Exact candidate checked occurrences; empty when the package left the closure.
    pub fn candidate_occurrence_contexts(&self) -> &[PackageCheckedContext] {
        &self.candidate_occurrence_contexts
    }
    /// Context changes remain audit-visible even when benign policy has no rows.
    pub fn occurrence_contexts_changed(&self) -> bool {
        self.baseline_occurrence_contexts != self.candidate_occurrence_contexts
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
    baseline_source_subject: Option<CanonicalSourceClosureSubjectFingerprint>,
    candidate_source_subject: CanonicalSourceClosureSubjectFingerprint,
    fingerprint: PackagePolicyChangeFingerprint,
    root_changed: bool,
    source_subject_changed: bool,
    root_role_change: Option<ReviewOnlyRootRoleChange>,
    source_replacements: Vec<PackagePolicySourceReplacement>,
    packages: Vec<PackagePolicyPackageChange>,
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
