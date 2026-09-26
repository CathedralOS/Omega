use super::super::{
    PackageLock, PackageLockError as Error, PackageLockRecoveryLimits, PackageLockTarget,
};
use super::{
    HEADER, MAXIMUM_DECISION_BYTES, MAXIMUM_POLICY_TEXT_BYTES, MAXIMUM_SOURCE_BYTES,
    budget::Budget, framing::Reader,
};
use crate::declarations::dependencies::DependencyPurpose;
use crate::lock::{PackageCheckedContext, PackagePolicyOccurrence};
use target::TargetProfile;

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
        if count > body.len() / "target \nsource 0\noccurrences 0\ndecisions 0\nend_target\n".len()
        {
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
            let maximum = source
                .packages()
                .len()
                .checked_mul(DependencyPurpose::ALL.len())
                .ok_or(Error::CountLimitExceeded)?;
            let count = reader.count("occurrences", maximum)?;
            budget.entries::<PackagePolicyOccurrence>(count)?;
            let mut occurrences = Vec::new();
            occurrences
                .try_reserve_exact(count)
                .map_err(|_| Error::AllocationFailed)?;
            for _ in 0..count {
                let row = reader.field("occurrence")?;
                let mut fields = row.split(' ');
                let index = fields.next().ok_or(Error::InvalidFraming)?;
                if index.is_empty()
                    || index.len() > 20
                    || (index.len() > 1 && index.starts_with('0'))
                    || !index.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(Error::InvalidFraming);
                }
                let index = index.parse::<usize>().map_err(|_| Error::InvalidFraming)?;
                let package = source
                    .packages()
                    .get(index)
                    .ok_or(Error::OccurrenceCoverage)?;
                let purpose_name = fields.next().ok_or(Error::InvalidFraming)?;
                let purpose = DependencyPurpose::ALL
                    .into_iter()
                    .find(|purpose| purpose.name() == purpose_name)
                    .ok_or(Error::InvalidFraming)?;
                let execution = fields.next().ok_or(Error::InvalidFraming)?;
                let execution = if execution == "none" {
                    None
                } else {
                    Some(profile_from_text(execution)?)
                };
                let checked_target =
                    profile_from_text(fields.next().ok_or(Error::InvalidFraming)?)?;
                if fields.next().is_some() {
                    return Err(Error::InvalidFraming);
                }
                let acceptance = budget.baseline(
                    reader.section("acceptance", MAXIMUM_POLICY_TEXT_BYTES)?,
                    package.key().identity(),
                    checked_target,
                )?;
                occurrences.push(PackagePolicyOccurrence {
                    acceptance,
                    context: PackageCheckedContext::new(purpose, checked_target, execution),
                });
            }
            let decisions = budget.decisions(
                reader.section("decisions", MAXIMUM_DECISION_BYTES)?,
                &source,
            )?;
            reader.expect("end_target")?;
            let target = PackageLockTarget::from_occurrences(source, occurrences, decisions)?;
            targets.push(target);
        }
        reader.expect("end")?;
        reader.finish()?;
        let value = Self { targets };
        value.validate(limits)?;
        Ok(value)
    }
}

fn profile_from_text(value: &str) -> Result<TargetProfile, Error> {
    TargetProfile::ALL
        .into_iter()
        .find(|profile| profile.identity().as_str() == value)
        .ok_or(Error::TargetMismatch)
}
