use super::{
    HistoricalPackagePolicyDecisions, PackageLockError as Error, PackageLockRecoveryLimits,
    PackageOccurrenceRoster,
};
use crate::declarations::PackageKey;
use crate::declarations::dependencies::DependencyPurpose;
use crate::resolution::graph::CanonicalSourceClosureSubject;
use package_evidence::record::PackagePolicyBaseline;
use target::TargetProfile;

/// Source pins, exact acceptance obligations, and historical project choices for
/// one exact target. This inert record is not fresh publication authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLockTarget {
    pub(super) source: CanonicalSourceClosureSubject,
    /// The authorized occurrence purposes of `source.packages()[i]`; baseline
    /// `i` consents to exactly these occurrences. This is the recorded join of
    /// each acceptance to the source graph's occurrence roster.
    pub(super) occurrence_purposes: Vec<Vec<DependencyPurpose>>,
    pub(super) baselines: Vec<super::PackagePolicyAcceptance>,
    pub(super) decisions: HistoricalPackagePolicyDecisions,
}

impl PackageLockTarget {
    /// Compose already constructed source and policy values without compiler,
    /// proof, or native replay. Baselines must follow exact source-package order.
    /// Every concrete policy owner must belong to the transitive source graph.
    pub fn from_parts(
        source: CanonicalSourceClosureSubject,
        baselines: Vec<PackagePolicyBaseline>,
        decisions: HistoricalPackagePolicyDecisions,
    ) -> Result<Self, Error> {
        // Validate incoming full policy before discarding reconstruction-only
        // associations. Compact rows are consent, never recovered compiler IR.
        if baselines.len() != source.packages().len() {
            return Err(Error::BaselineCoverage);
        }
        let limits = PackageLockRecoveryLimits::default();
        super::validation::policy_source_membership(
            &source,
            &baselines,
            limits.maximum_owned_bytes,
            limits.maximum_identity_nodes,
        )?;
        let baselines = baselines
            .iter()
            .map(super::PackagePolicyAcceptance::from_policy)
            .collect::<Result<_, _>>()?;
        Self::from_acceptances(source, baselines, decisions)
    }

    /// Compose retained consent without reconstructing or cloning old compiler policy.
    pub fn from_acceptances(
        source: CanonicalSourceClosureSubject,
        baselines: Vec<super::PackagePolicyAcceptance>,
        decisions: HistoricalPackagePolicyDecisions,
    ) -> Result<Self, Error> {
        let occurrence_purposes = Self::derived_occurrence_purposes(&source)?;
        Self::from_recorded_parts(source, occurrence_purposes, baselines, decisions)
    }

    /// Compose a target whose occurrence coverage was recovered from text;
    /// validation still requires the recorded coverage to equal the roster the
    /// source graph derives.
    pub(super) fn from_recorded_parts(
        source: CanonicalSourceClosureSubject,
        occurrence_purposes: Vec<Vec<DependencyPurpose>>,
        baselines: Vec<super::PackagePolicyAcceptance>,
        decisions: HistoricalPackagePolicyDecisions,
    ) -> Result<Self, Error> {
        let value = Self {
            source,
            occurrence_purposes,
            baselines,
            decisions,
        };
        value.validate()?;
        Ok(value)
    }

    /// Every package's authorized occurrence purposes, in `source.packages()`
    /// order. Baseline `i` answers exactly these occurrences.
    pub fn occurrence_purposes(&self) -> &[Vec<DependencyPurpose>] {
        &self.occurrence_purposes
    }

    /// The purposes one package occurs under in this target's closure.
    pub fn occurrence_purposes_for(&self, package: &PackageKey) -> Option<&[DependencyPurpose]> {
        self.source
            .packages()
            .binary_search_by(|source| source.key().cmp(package))
            .ok()
            .map(|index| self.occurrence_purposes[index].as_slice())
    }

    pub(super) fn derived_occurrence_purposes(
        source: &CanonicalSourceClosureSubject,
    ) -> Result<Vec<Vec<DependencyPurpose>>, Error> {
        Ok(PackageOccurrenceRoster::derive(source)
            .map_err(|error| match error {
                super::PackageOccurrenceRosterError::AllocationFailed => Error::AllocationFailed,
                _ => Error::OccurrenceCoverage,
            })?
            .coverages()
            .iter()
            .map(|coverage| coverage.purposes().to_vec())
            .collect())
    }

    pub fn source(&self) -> &CanonicalSourceClosureSubject {
        &self.source
    }
    pub fn baselines(&self) -> &[super::PackagePolicyAcceptance] {
        &self.baselines
    }
    pub fn decisions(&self) -> &HistoricalPackagePolicyDecisions {
        &self.decisions
    }
    pub fn target(&self) -> TargetProfile {
        self.source.target_profile()
    }

    // Cheap joins only. The public constructor and budgeted text reader also
    // establish complete policy membership before exposing an immutable target.
    pub(super) fn validate(&self) -> Result<(), Error> {
        let derived = Self::derived_occurrence_purposes(&self.source)?;
        if self.occurrence_purposes != derived {
            return Err(Error::OccurrenceCoverage);
        }
        if self.baselines.len() != self.source.packages().len()
            || self
                .baselines
                .iter()
                .zip(self.source.packages())
                .any(|(baseline, source)| baseline.package() != source.key().identity())
        {
            return Err(Error::BaselineCoverage);
        }
        if self
            .baselines
            .iter()
            .any(|baseline| baseline.target() != self.target())
        {
            return Err(Error::TargetMismatch);
        }
        if self.decisions.source_subject() != self.source.fingerprint() {
            return Err(Error::DecisionSourceMismatch);
        }
        Ok(())
    }
}

/// Deterministic retained project state for explicitly listed targets. The
/// immutable source graph is identical across sections; target-sensitive policy
/// and decisions remain separate. No section authorizes another target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLock {
    pub(super) targets: Vec<PackageLockTarget>,
}

impl PackageLock {
    pub fn from_targets(targets: Vec<PackageLockTarget>) -> Result<Self, Error> {
        let value = Self { targets };
        value.validate(PackageLockRecoveryLimits::default())?;
        Ok(value)
    }

    pub fn targets(&self) -> &[PackageLockTarget] {
        &self.targets
    }

    /// Retain one exact target's accepted policy without copying other sections.
    pub fn into_target(self, profile: TargetProfile) -> Option<PackageLockTarget> {
        self.targets
            .into_iter()
            .find(|target| target.target() == profile)
    }

    pub fn target(&self, profile: TargetProfile) -> Option<&PackageLockTarget> {
        self.targets
            .binary_search_by(|target| {
                target
                    .target()
                    .identity()
                    .as_str()
                    .cmp(profile.identity().as_str())
            })
            .ok()
            .map(|index| &self.targets[index])
    }

    pub(super) fn validate(&self, limits: PackageLockRecoveryLimits) -> Result<(), Error> {
        let Some(first) = self.targets.first() else {
            return Err(Error::EmptyTargets);
        };
        if self.targets.len() > limits.maximum_targets {
            return Err(Error::CountLimitExceeded);
        }
        if self.targets.windows(2).any(|pair| {
            pair[0].target().identity().as_str() >= pair[1].target().identity().as_str()
        }) {
            return Err(Error::TargetOrder);
        }
        let mut packages = 0usize;
        let mut requests = 0usize;
        let mut decisions = 0usize;
        for target in &self.targets {
            target.validate()?;
            if !first.source.same_source_graph(&target.source) {
                return Err(Error::SourceGraphMismatch);
            }
            packages = packages
                .checked_add(target.source.packages().len())
                .ok_or(Error::CountLimitExceeded)?;
            decisions = decisions
                .checked_add(target.decisions.decisions().len())
                .ok_or(Error::CountLimitExceeded)?;
            requests = requests
                .checked_add(target.source.dependency_requests().len())
                .ok_or(Error::CountLimitExceeded)?;
            for package in target.source.packages() {
                let projection = target
                    .source
                    .package_dependency_projection(package.key())
                    .ok_or(Error::BaselineCoverage)?;
                requests = requests
                    .checked_add(projection.authored_request_count())
                    .ok_or(Error::CountLimitExceeded)?;
            }
        }
        if packages > limits.maximum_packages
            || requests > limits.maximum_dependency_requests
            || decisions > limits.maximum_decisions
        {
            return Err(Error::CountLimitExceeded);
        }
        Ok(())
    }
}
