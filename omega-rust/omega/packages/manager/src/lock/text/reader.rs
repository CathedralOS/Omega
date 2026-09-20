use super::super::{
    PackageLock, PackageLockError as Error, PackageLockRecoveryLimits, PackageLockTarget,
};
use super::{
    HEADER, MAXIMUM_DECISION_BYTES, MAXIMUM_POLICY_TEXT_BYTES, MAXIMUM_SOURCE_BYTES,
    budget::Budget, framing::Reader,
};
use crate::declarations::dependencies::DependencyPurpose;
use crate::lock::PackagePolicyAcceptance;
use target::TargetProfile;

impl PackageLock {
    /// Recover project records without filesystem access, source acquisition,
    /// compiler execution, or conversion of history into fresh authorization.
    pub fn recover_text(text: &str, limits: PackageLockRecoveryLimits) -> Result<Self, Error> {
        let limits = limits.bounded();
        if text.len() > limits.maximum_bytes {
            return Err(Error::ByteLimitExceeded);
        }
        // A known v1 snapshot can be projected without reacquiring or compiling
        // its source. Unknown schemas never get guessed-equivalent acceptance.
        let (body, snapshot) = if let Some(body) = text.strip_prefix(HEADER) {
            (body, false)
        } else if let Some(body) = text.strip_prefix("omega_lock 1\n") {
            (body, true)
        } else {
            return Err(Error::UnsupportedVersion);
        };
        let mut reader = Reader::new(body);
        let count = reader.count("targets", limits.maximum_targets)?;
        if count == 0 {
            return Err(Error::EmptyTargets);
        }
        // Reject impossible framing before requesting semantic storage.
        if count > body.len() / "target \nsource 0\nacceptances 0\ndecisions 0\nend_target\n".len()
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
            // The v2 occurrence ledger records the derived roster; locks written
            // before it carry implicit complete coverage, so absence assigns
            // the derived roster rather than guessed acceptance. A snapshot
            // projection of a v2 frame still carries and consumes the ledger.
            let occurrence_purposes = if !reader.starts_with("occurrences ") {
                PackageLockTarget::derived_occurrence_purposes(&source)?
            } else {
                let maximum = source
                    .packages()
                    .len()
                    .checked_mul(DependencyPurpose::ALL.len())
                    .ok_or(Error::CountLimitExceeded)?;
                let rows = reader.count("occurrences", maximum)?;
                let mut coverage = Vec::<Vec<DependencyPurpose>>::new();
                coverage
                    .try_reserve_exact(source.packages().len())
                    .map_err(|_| Error::AllocationFailed)?;
                coverage.resize_with(source.packages().len(), Vec::new);
                let mut previous: Option<(usize, DependencyPurpose)> = None;
                for _ in 0..rows {
                    let row = reader.field("occurrence")?;
                    let (index, purpose) = row.split_once(' ').ok_or(Error::InvalidFraming)?;
                    if index.len() > 20
                        || (index.len() > 1 && index.starts_with('0'))
                        || !index.bytes().all(|byte| byte.is_ascii_digit())
                    {
                        return Err(Error::InvalidFraming);
                    }
                    let index = index.parse::<usize>().map_err(|_| Error::InvalidFraming)?;
                    let purpose = DependencyPurpose::ALL
                        .iter()
                        .copied()
                        .find(|candidate| candidate.name() == purpose)
                        .ok_or(Error::InvalidFraming)?;
                    let position = (index, purpose);
                    if previous.is_some_and(|last| position <= last) {
                        return Err(Error::InvalidFraming);
                    }
                    previous = Some(position);
                    coverage
                        .get_mut(index)
                        .ok_or(Error::InvalidFraming)?
                        .push(purpose);
                }
                coverage
            };
            let count = reader.count(
                if snapshot { "baselines" } else { "acceptances" },
                source.packages().len(),
            )?;
            if count != source.packages().len() {
                return Err(Error::BaselineCoverage);
            }
            budget.entries::<PackagePolicyAcceptance>(count)?;
            let mut baselines = Vec::new();
            baselines
                .try_reserve_exact(count)
                .map_err(|_| Error::AllocationFailed)?;
            for package in source.packages() {
                let baseline = if snapshot {
                    budget.snapshot(
                        reader.section("baseline", MAXIMUM_POLICY_TEXT_BYTES)?,
                        &source,
                    )?
                } else {
                    budget.baseline(
                        reader.section("acceptance", MAXIMUM_POLICY_TEXT_BYTES)?,
                        package.key().identity(),
                        profile,
                    )?
                };
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
            let target = PackageLockTarget::from_recorded_parts(
                source,
                occurrence_purposes,
                baselines,
                decisions,
            )?;
            targets.push(target);
        }
        reader.expect("end")?;
        reader.finish()?;
        let value = Self { targets };
        value.validate(limits)?;
        Ok(value)
    }
}
