use super::{
    HistoricalPackagePolicyDecisions, PackageCheckedContext, PackageLockError as Error,
    PackageLockRecoveryLimits, PackageOccurrenceRoster, PackagePolicyOccurrence,
};
use crate::declarations::dependencies::DependencyPurpose;
use crate::resolution::graph::CanonicalSourceClosureSubject;
use package_evidence::record::PackagePolicyBaseline;
use target::TargetProfile;

/// Source pins and independently retained consent for every checked occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLockTarget {
    pub(super) source: CanonicalSourceClosureSubject,
    pub(super) occurrences: Vec<PackagePolicyOccurrence>,
    pub(super) decisions: HistoricalPackagePolicyDecisions,
}

impl PackageLockTarget {
    /// Validate full policy associations before projecting compact historical intent.
    pub fn from_policies(
        source: CanonicalSourceClosureSubject,
        policies: &[(PackageCheckedContext, &PackagePolicyBaseline)],
        decisions: HistoricalPackagePolicyDecisions,
    ) -> Result<Self, Error> {
        let occurrences = policies
            .iter()
            .map(|(context, policy)| PackagePolicyOccurrence::from_policy(policy, *context))
            .collect::<Result<Vec<_>, _>>()?;
        let value = Self::from_occurrences(source, occurrences, decisions)?;
        let limits = PackageLockRecoveryLimits::default();
        super::validation::policy_source_membership(
            &value.source,
            policies,
            limits.maximum_owned_bytes,
            limits.maximum_identity_nodes,
        )?;
        Ok(value)
    }

    /// Compose inert compact consent; no compiler facts are recovered from it.
    pub fn from_occurrences(
        source: CanonicalSourceClosureSubject,
        occurrences: Vec<PackagePolicyOccurrence>,
        decisions: HistoricalPackagePolicyDecisions,
    ) -> Result<Self, Error> {
        let value = Self {
            source,
            occurrences,
            decisions,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn source(&self) -> &CanonicalSourceClosureSubject {
        &self.source
    }
    pub fn occurrences(&self) -> &[PackagePolicyOccurrence] {
        &self.occurrences
    }
    pub fn decisions(&self) -> &HistoricalPackagePolicyDecisions {
        &self.decisions
    }
    pub fn target(&self) -> TargetProfile {
        self.source.target_profile()
    }

    /// All occurrence records have been checked to agree on this explicit profile.
    pub fn execution_profile(&self) -> Option<TargetProfile> {
        self.occurrences
            .first()
            .and_then(|occurrence| occurrence.context().build_execution_profile())
    }

    pub(super) fn validate(&self) -> Result<(), Error> {
        let roster =
            PackageOccurrenceRoster::derive(&self.source).map_err(|error| match error {
                super::PackageOccurrenceRosterError::AllocationFailed => Error::AllocationFailed,
                _ => Error::OccurrenceCoverage,
            })?;
        if self.occurrences.len() != roster.occurrence_count() {
            return Err(Error::OccurrenceCoverage);
        }
        let mut occurrences = self.occurrences.iter();
        for coverage in roster.coverages() {
            for purpose in coverage.purposes() {
                let occurrence = occurrences.next().ok_or(Error::OccurrenceCoverage)?;
                let context = occurrence.context();
                if occurrence.acceptance().package() != coverage.package().identity()
                    || context.purpose() != *purpose
                {
                    return Err(Error::OccurrenceCoverage);
                }
                occurrence.validate()?;
                if context.build_execution_profile() != self.execution_profile() {
                    return Err(Error::ExecutionProfileMismatch);
                }
                if *purpose == DependencyPurpose::Product && context.target() != self.target() {
                    return Err(Error::TargetMismatch);
                }
            }
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
                    .ok_or(Error::SourceCoverage)?;
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
