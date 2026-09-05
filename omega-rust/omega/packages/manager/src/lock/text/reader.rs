use super::super::{
    PackageLock, PackageLockError as Error, PackageLockRecoveryLimits, PackageLockTarget,
};
use super::{
    HEADER, MAXIMUM_DECISION_BYTES, MAXIMUM_POLICY_TEXT_BYTES, MAXIMUM_SOURCE_BYTES,
    budget::Budget, framing::Reader,
};
use omega_package_evidence::record::PackagePolicyBaseline;
use omega_target::TargetProfile;

impl PackageLock {
    /// Recover project records without filesystem access, source acquisition,
    /// compiler execution, or conversion of history into fresh authorization.
    pub fn recover_text(text: &str, limits: PackageLockRecoveryLimits) -> Result<Self, Error> {
        let limits = limits.bounded();
        if text.len() > limits.maximum_bytes {
            return Err(Error::ByteLimitExceeded);
        }
        let body = text.strip_prefix(HEADER).ok_or(Error::UnsupportedVersion)?;
        let mut reader = Reader::new(body);
        let count = reader.count("targets", limits.maximum_targets)?;
        if count == 0 {
            return Err(Error::EmptyTargets);
        }
        // Reject impossible framing before requesting semantic storage.
        if count > body.len() / "target \nsource 0\nbaselines 0\ndecisions 0\nend_target\n".len() {
            return Err(Error::InvalidFraming);
        }
        let mut budget = Budget::new(limits);
        budget.entries::<PackageLockTarget>(count)?;
        let mut targets = Vec::<PackageLockTarget>::new();
        targets
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        for _ in 0..count {
            let profile = reader.field("target")?;
            let profile = TargetProfile::ALL
                .into_iter()
                .find(|target| target.identity().as_str() == profile)
                .ok_or(Error::TargetMismatch)?;
            if targets.last().is_some_and(|last| {
                last.target().identity().as_str() >= profile.identity().as_str()
            }) {
                return Err(Error::TargetOrder);
            }
            let source = budget.source(reader.section("source", MAXIMUM_SOURCE_BYTES)?)?;
            if source.target_profile() != profile {
                return Err(Error::TargetMismatch);
            }
            if targets
                .first()
                .is_some_and(|first| !first.source.same_source_graph(&source))
            {
                return Err(Error::SourceGraphMismatch);
            }
            let count = reader.count("baselines", source.packages().len())?;
            if count != source.packages().len() {
                return Err(Error::BaselineCoverage);
            }
            budget.entries::<PackagePolicyBaseline>(count)?;
            let mut baselines = Vec::new();
            baselines
                .try_reserve_exact(count)
                .map_err(|_| Error::AllocationFailed)?;
            for package in source.packages() {
                let baseline =
                    budget.baseline(reader.section("baseline", MAXIMUM_POLICY_TEXT_BYTES)?)?;
                if baseline.package() != package.key().identity() {
                    return Err(Error::BaselineCoverage);
                }
                if baseline.target() != profile {
                    return Err(Error::TargetMismatch);
                }
                baselines.push(baseline);
            }
            let decisions = budget.decisions(
                reader.section("decisions", MAXIMUM_DECISION_BYTES)?,
                &source,
            )?;
            reader.expect("end_target")?;
            targets.push(PackageLockTarget::from_parts(source, baselines, decisions)?);
        }
        reader.expect("end")?;
        reader.finish()?;
        let value = Self { targets };
        value.validate(limits)?;
        Ok(value)
    }
}
