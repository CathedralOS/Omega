use super::{
    HistoricalPackagePolicyDecisions, PackageLockError as Error, PackageLockRecoveryLimits,
};
use crate::resolution::graph::CanonicalSourceClosureSubject;
use omega_package_evidence::record::PackagePolicyBaseline;
use omega_target::TargetProfile;

/// Source pins, complete normalized policy, and historical project choices for
/// one exact target. This inert record is not fresh publication authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLockTarget {
    pub(super) source: CanonicalSourceClosureSubject,
    pub(super) baselines: Vec<PackagePolicyBaseline>,
    pub(super) decisions: HistoricalPackagePolicyDecisions,
}

impl PackageLockTarget {
    /// Compose already constructed source and policy values without compiler,
    /// proof, or native replay. Baselines must follow exact source-package order.
    pub fn from_parts(
        source: CanonicalSourceClosureSubject,
        baselines: Vec<PackagePolicyBaseline>,
        decisions: HistoricalPackagePolicyDecisions,
    ) -> Result<Self, Error> {
        let value = Self {
            source,
            baselines,
            decisions,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn source(&self) -> &CanonicalSourceClosureSubject {
        &self.source
    }
    pub fn baselines(&self) -> &[PackagePolicyBaseline] {
        &self.baselines
    }
    pub fn decisions(&self) -> &HistoricalPackagePolicyDecisions {
        &self.decisions
    }
    pub fn target(&self) -> TargetProfile {
        self.source.target_profile()
    }

    pub(super) fn validate(&self) -> Result<(), Error> {
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
                    .checked_add(projection.authored_dependencies().len())
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
