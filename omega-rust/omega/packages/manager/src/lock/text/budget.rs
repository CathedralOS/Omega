//! One accounting path for recovery and serialization's recoverability check.

use super::super::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLockError as Error,
    PackageLockRecoveryLimits,
};
use super::MAXIMUM_DECISION_BYTES;
use crate::lock::PackagePolicyAcceptance;
use crate::resolution::graph::CanonicalSourceClosureSubject;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub(super) struct Budget {
    remaining: PackageLockRecoveryLimits,
}

impl Budget {
    pub(super) fn new(limits: PackageLockRecoveryLimits) -> Self {
        Self { remaining: limits }
    }

    pub(super) fn entries<T>(&mut self, count: usize) -> Result<(), Error> {
        self.owned(
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(Error::AllocationLimitExceeded)?,
        )
    }

    fn owned(&mut self, count: usize) -> Result<(), Error> {
        self.remaining.maximum_owned_bytes = self
            .remaining
            .maximum_owned_bytes
            .checked_sub(count)
            .ok_or(Error::AllocationLimitExceeded)?;
        Ok(())
    }

    pub(super) fn source(&mut self, text: &str) -> Result<CanonicalSourceClosureSubject, Error> {
        let (source, usage) = CanonicalSourceClosureSubject::recover_text_with_usage(
            text,
            self.remaining.source_limits(),
            self.remaining.maximum_owned_bytes,
        )
        .map_err(Error::Source)?;
        self.owned(usage.owned_bytes())?;
        count(&mut self.remaining.maximum_packages, usage.packages())?;
        count(
            &mut self.remaining.maximum_dependency_requests,
            usage.authored_dependency_requests(),
        )?;
        count(
            &mut self.remaining.maximum_dependency_requests,
            usage.dependency_requests(),
        )?;
        Ok(source)
    }

    pub(super) fn baseline(
        &mut self,
        text: &str,
        package: PackageKeyIdentity,
        target: TargetProfile,
    ) -> Result<PackagePolicyAcceptance, Error> {
        let (baseline, owned) = super::acceptance::read(
            text,
            package,
            target,
            self.remaining.maximum_policy_elements,
            self.remaining.maximum_owned_bytes,
        )?;
        self.owned(owned)?;
        count(
            &mut self.remaining.maximum_policy_elements,
            baseline.rows().len(),
        )?;
        Ok(baseline)
    }

    // Read-only migration of the one known historical snapshot schema. The
    // existing evidence decoder checks its full structure before projection.
    pub(super) fn snapshot(
        &mut self,
        text: &str,
        source: &CanonicalSourceClosureSubject,
    ) -> Result<PackagePolicyAcceptance, Error> {
        use package_evidence::{
            encoding::{PackagePolicyRecoveryLimits, PackagePolicyTextRecoveryLimits},
            record::PackagePolicyBaseline,
        };
        let limits = PackagePolicyRecoveryLimits::new(
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            self.remaining.maximum_policy_elements,
            self.remaining.maximum_owned_bytes,
            128,
        );
        let (policy, usage) = PackagePolicyBaseline::recover_text_with_usage(
            text,
            PackagePolicyTextRecoveryLimits::new(super::MAXIMUM_POLICY_TEXT_BYTES, limits),
        )
        .map_err(Error::Policy)?;
        self.owned(usage.owned_bytes())?;
        count(
            &mut self.remaining.maximum_policy_elements,
            usage.sequence_elements(),
        )?;
        let membership = policy
            .validate_package_membership(
                |identity| {
                    source
                        .packages()
                        .iter()
                        .any(|package| package.key().identity() == identity)
                },
                package_evidence::encoding::PackagePolicyMembershipLimits::new(
                    self.remaining.maximum_owned_bytes,
                    self.remaining.maximum_identity_nodes,
                    128,
                ),
            )
            .map_err(Error::PolicySourceMembership)?;
        self.owned(membership.owned_bytes())?;
        count(
            &mut self.remaining.maximum_identity_nodes,
            membership.identity_nodes(),
        )?;
        let (acceptance, projected) = PackagePolicyAcceptance::project(
            &policy,
            package_evidence::record::PackagePolicyRowLimits {
                maximum_rows: self.remaining.maximum_policy_elements,
                maximum_owned_bytes: self.remaining.maximum_owned_bytes,
                maximum_sequence_elements: self.remaining.maximum_policy_elements,
                ..Default::default()
            },
        )
        .map_err(Error::Encoding)?;
        self.owned(projected.owned_bytes())?;
        count(
            &mut self.remaining.maximum_policy_elements,
            projected.sequence_elements(),
        )?;
        Ok(acceptance)
    }

    pub(super) fn decisions(
        &mut self,
        text: &str,
        source: &CanonicalSourceClosureSubject,
    ) -> Result<HistoricalPackagePolicyDecisions, Error> {
        let (decisions, usage) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
            text,
            source,
            HistoricalPackagePolicyLimits::new(
                MAXIMUM_DECISION_BYTES,
                self.remaining.maximum_decisions,
            ),
            self.remaining.maximum_owned_bytes,
        )
        .map_err(Error::Decisions)?;
        self.owned(usage.owned_bytes())?;
        count(&mut self.remaining.maximum_decisions, usage.decisions())?;
        Ok(decisions)
    }
}

fn count(remaining: &mut usize, used: usize) -> Result<(), Error> {
    *remaining = remaining
        .checked_sub(used)
        .ok_or(Error::CountLimitExceeded)?;
    Ok(())
}
