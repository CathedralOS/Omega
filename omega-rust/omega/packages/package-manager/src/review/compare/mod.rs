/*
    We compare one complete fresh candidate review set against retained lock
    policy or an explicit empty initial baseline (`compare_package_policy_changes`),
    producing a normalized `PackagePolicyChangeSet` without reacquiring old
    sources or replaying the compiler.

    Comparison binds both inert policy rows and fresh issuer provenance into
    domain-separated `PackagePolicyChangeFingerprint`s (`fingerprints`), merges
    baseline and candidate occurrences per package and purpose (`assembly`,
    `merge`, `projection`), derives shortest canonical dependency paths
    (`paths`), and identifies root-role contract breaks (`ReviewOnlyRootRoleChange`)
    and requester-local source replacements (`replacements`) under aggregate
    `Budget` ceilings (`limits`).
*/

/*
    Policy comparison belongs to the package review comparison boundary
    (`review/compare/mod.rs`).
*/

mod assembly;
mod fingerprints;
mod limits;
mod merge;
mod paths;
mod projection;
mod replacements;

pub(crate) use limits::PackagePolicyChangeLimits;
pub(crate) use replacements::{PackagePolicyReplacementSite, PackagePolicySourceReplacement};

use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::declarations::dependencies::DependencyPurpose;
use crate::lock::PackageLockTarget;
use crate::lock::{PackageAcceptanceRow, PackageCheckedContext, PackageOccurrenceRosterError};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectError,
    CanonicalSourceClosureSubjectFingerprint, ExactTargetPackageSourceClosure,
};
use crate::review::CompilerIssuedPackageReviewSet;
use crate::review::timings;
use limits::Budget;
use package_compilation::build_execution::RestrictedBuildRequest;
use package_evidence::encoding::PackageReviewEncodingError;
use package_evidence::record::PackagePolicyRowKind;
use package_source::ImmutableSourceResolution;
use semantic_vocabulary::PackageKeyIdentity;
use sha2::Digest;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ReviewOnlyRootRoleContract {
    DependencyCompatibility,
    ApplicationActivation,
}

impl ReviewOnlyRootRoleContract {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DependencyCompatibility => "dependency-compatibility",
            Self::ApplicationActivation => "application-activation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewOnlyRootRoleChange {
    root: PackageKey,
    baseline_role: BuildDeclarationKind,
    candidate_role: BuildDeclarationKind,
    broken_contract: ReviewOnlyRootRoleContract,
}

impl ReviewOnlyRootRoleChange {
    pub fn root(&self) -> &PackageKey {
        &self.root
    }

    pub const fn baseline_role(&self) -> BuildDeclarationKind {
        self.baseline_role
    }

    pub const fn candidate_role(&self) -> BuildDeclarationKind {
        self.candidate_role
    }

    pub const fn broken_contract(&self) -> ReviewOnlyRootRoleContract {
        self.broken_contract
    }
}

#[derive(Debug)]
pub(crate) enum PackagePolicyChangeError {
    TargetMismatch,
    CandidateReview {
        package: Option<Box<PackageKey>>,
        reason: &'static str,
    },
    SourceSubject(CanonicalSourceClosureSubjectError),
    Projection {
        package: Box<PackageKey>,
        error: PackageReviewEncodingError,
    },
    InvalidSourcePath {
        package: Box<PackageKey>,
    },
    OccurrenceRoster,
    LimitExceeded {
        resource: &'static str,
        maximum: usize,
    },
    AllocationFailed,
}

impl fmt::Display for PackagePolicyChangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMismatch => {
                formatter.write_str("normalized policy comparison requires one exact target")
            }
            Self::CandidateReview { package, reason } => write!(
                formatter,
                "invalid fresh candidate review {package:?}: {reason}"
            ),
            Self::SourceSubject(error) => {
                write!(formatter, "invalid candidate source subject: {error}")
            }
            Self::Projection { package, error } => write!(
                formatter,
                "cannot project normalized policy rows for {package:?}: {error}"
            ),
            Self::InvalidSourcePath { package } => write!(
                formatter,
                "source subject has no bounded path to {package:?}"
            ),
            Self::OccurrenceRoster => {
                formatter.write_str("source subject does not yield a complete occurrence roster")
            }
            Self::LimitExceeded { resource, maximum } => write!(
                formatter,
                "normalized policy comparison exceeds {resource} limit {maximum}"
            ),
            Self::AllocationFailed => {
                formatter.write_str("normalized policy comparison allocation failed")
            }
        }
    }
}

impl std::error::Error for PackagePolicyChangeError {}

impl From<PackageOccurrenceRosterError> for PackagePolicyChangeError {
    fn from(error: PackageOccurrenceRosterError) -> Self {
        match error {
            PackageOccurrenceRosterError::AllocationFailed => Self::AllocationFailed,
            PackageOccurrenceRosterError::UnknownPackage
            | PackageOccurrenceRosterError::UnreachablePackage => Self::OccurrenceRoster,
        }
    }
}

pub(crate) fn compare_package_policy_changes(
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
pub(crate) enum PackagePolicyChangeKind {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PackagePolicyChangeFingerprint([u8; 32]);
impl PackagePolicyChangeFingerprint {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyRowChange {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyDependencyPath {
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
pub(crate) struct PackagePolicyDependencyPathStep {
    requester: PackageKeyIdentity,
    purpose: DependencyPurpose,
    dependency_index: usize,
    alias: String,
    target: PackageKeyIdentity,
}
impl PackagePolicyDependencyPathStep {
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
pub(crate) struct PackagePolicyPackageChange {
    key: PackageKey,
    baseline_resolution: Option<ImmutableSourceResolution>,
    candidate_resolution: Option<ImmutableSourceResolution>,
    baseline_path: Option<PackagePolicyDependencyPath>,
    candidate_path: Option<PackagePolicyDependencyPath>,
    restricted_build_requests: Vec<RestrictedBuildRequest>, // Normalized restricted build-host requests surfaced beside product-authority rows.
    baseline_occurrence_contexts: Vec<PackageCheckedContext>, // Occurrences the accepted baseline consented to.
    candidate_occurrence_contexts: Vec<PackageCheckedContext>, // Occurrences the candidate review answers for.
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
    pub fn restricted_build_requests(&self) -> &[RestrictedBuildRequest] {
        &self.restricted_build_requests
    }
    pub(crate) fn baseline_occurrence_contexts(&self) -> &[PackageCheckedContext] {
        &self.baseline_occurrence_contexts
    }
    pub(crate) fn candidate_occurrence_contexts(&self) -> &[PackageCheckedContext] {
        &self.candidate_occurrence_contexts
    }
    pub(crate) fn occurrence_contexts_changed(&self) -> bool {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyChangeSet {
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
    pub(crate) fn audit_recommended(&self) -> bool {
        self.source_subject_changed
            || self
                .packages
                .iter()
                .any(PackagePolicyPackageChange::audit_recommended)
    }

    pub(crate) fn source_replacements(&self) -> &[PackagePolicySourceReplacement] {
        &self.source_replacements
    }
}
