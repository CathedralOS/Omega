use super::super::PackageLockTarget;
use super::super::{PackageLock, PackageLockError as Error, PackageLockRecoveryLimits};
use super::{HEADER, budget::Budget, framing::Writer};
use omega_package_evidence::record::PackagePolicyBaseline;

impl PackageLock {
    /// Diffable child texts remain verbatim, with explicit byte lengths to
    /// delimit them. No whole child becomes an opaque escaped payload.
    pub fn canonical_text(&self) -> Result<String, Error> {
        self.canonical_text_with_limits(PackageLockRecoveryLimits::default())
    }

    /// Emit only records recoverable under these ceilings. Each child is
    /// decoded once for exact recovery accounting and immediately dropped;
    /// historical decision emission additionally accounts its temporary text
    /// and validation storage. No duplicate full lock or acceptance certificate
    /// is constructed.
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
        let mut remaining_elements = limits.maximum_policy_elements;
        for target in &self.targets {
            writer.row("target", target.target().identity().as_str())?;
            let source = target
                .source
                .canonical_text(limits.source_limits())
                .map_err(Error::Source)?;
            drop(budget.source(&source)?);
            writer.section("source", &source)?;
            drop(source);
            writer.row("baselines", target.baselines.len())?;
            budget.entries::<PackagePolicyBaseline>(target.baselines.len())?;
            for policy in &target.baselines {
                let (text, elements) = policy
                    .canonical_text_with_element_count()
                    .map_err(Error::Encoding)?;
                remaining_elements = remaining_elements
                    .checked_sub(elements)
                    .ok_or(Error::CountLimitExceeded)?;
                drop(budget.baseline(&text)?);
                writer.section("baseline", &text)?;
            }
            let decisions = budget.decision_text(&target.decisions, &target.source)?;
            budget.target_membership(target)?;
            writer.section("decisions", &decisions)?;
            writer.append("end_target\n")?;
        }
        writer.append("end\n")?;
        writer.finish()
    }
}
