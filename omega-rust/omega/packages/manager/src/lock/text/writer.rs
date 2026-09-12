use super::super::PackageLockTarget;
use super::super::{
    HistoricalPackagePolicyLimits, PackageLock, PackageLockError as Error,
    PackageLockRecoveryLimits,
};
use super::{HEADER, budget::Budget, framing::Writer};
use crate::lock::PackagePolicyAcceptance;

impl PackageLock {
    /// Diffable child texts remain verbatim, with explicit byte lengths to
    /// delimit them. No whole child becomes an opaque escaped payload.
    pub fn canonical_text(&self) -> Result<String, Error> {
        self.canonical_text_with_limits(PackageLockRecoveryLimits::default())
    }

    /// Emit only records recoverable under these ceilings. Each child is
    /// decoded once for exact resource accounting and immediately dropped;
    /// no duplicate full lock or acceptance certificate is constructed.
    pub fn canonical_text_with_limits(
        &self,
        limits: PackageLockRecoveryLimits,
    ) -> Result<String, Error> {
        let limits = limits.bounded();
        self.validate(limits)?;
        let mut budget = Budget::new(limits);
        budget.entries::<PackageLockTarget>(self.targets.len())?;
        let mut writer = Writer::new(limits.maximum_bytes);
        writer.append(HEADER)?;
        writer.row("targets", self.targets.len())?;
        for target in &self.targets {
            writer.row("target", target.target().identity().as_str())?;
            let source = target
                .source
                .canonical_text(limits.source_limits())
                .map_err(Error::Source)?;
            drop(budget.source(&source)?);
            writer.section("source", &source)?;
            drop(source);
            writer.row("acceptances", target.baselines.len())?;
            budget.entries::<PackagePolicyAcceptance>(target.baselines.len())?;
            for policy in &target.baselines {
                let text = policy.canonical_text()?;
                drop(budget.baseline(&text, policy.package(), policy.target())?);
                writer.section("acceptance", &text)?;
            }
            let decisions = target
                .decisions
                .canonical_text(&target.source, HistoricalPackagePolicyLimits::default())
                .map_err(Error::Decisions)?;
            drop(budget.decisions(&decisions, &target.source)?);
            writer.section("decisions", &decisions)?;
            writer.append("end_target\n")?;
        }
        writer.append("end\n")?;
        writer.finish()
    }
}
