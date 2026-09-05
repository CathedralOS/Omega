//! Callable authority and checked behavior without decoding nominal identities.

use super::*;

impl Names<'_> {
    pub(super) fn callable(
        &self,
        output: &mut Output,
        callable: &PackagePolicyCallable,
    ) -> fmt::Result {
        writeln!(
            output,
            "  callable {} role {:?} supply {:?}",
            self.name(callable.identity()),
            callable.role(),
            callable.supply()
        )?;
        self.reach(
            output,
            "declared-reach",
            callable.declared_service_reach(),
            "not declared",
        )?;
        self.reach(
            output,
            "checked-reach",
            callable.checked_service_reach().realized(),
            "unknown (no checked body)",
        )?;
        self.reach(
            output,
            "concrete-reach",
            callable.checked_service_reach().concrete(),
            "unknown (no checked body)",
        )?;
        self.invocations(
            output,
            "declared-invocations",
            callable.declared_synchronous_invocations(),
            "not declared",
        )?;
        let checked = callable.checked_service_reach().realized().is_some();
        self.invocations(
            output,
            "checked-invocations",
            checked.then_some(callable.realized_synchronous_invocations()),
            "unknown (no checked body)",
        )?;
        writeln!(
            output,
            "    declared-effects suspend {:?} block {:?} (None = not declared)",
            callable.declared_may_suspend(),
            callable.declared_may_block()
        )?;
        if checked {
            writeln!(
                output,
                "    checked-effects suspend {} block {}",
                callable.checked_may_suspend(),
                callable.checked_may_block()
            )?;
            writeln!(
                output,
                "    checked-termination {:?}",
                callable.checked_termination()
            )?;
            writeln!(
                output,
                "    checked-crash {:?}",
                callable.checked_crash().inferred()
            )?;
        } else {
            writeln!(output, "    checked-effects unknown (no checked body)")?;
            writeln!(output, "    checked-termination unknown (no checked body)")?;
            writeln!(output, "    checked-crash unknown (no checked body)")?;
        }
        writeln!(
            output,
            "    declared-termination {:?} (None = not declared)",
            callable.declared_termination()
        )?;
        writeln!(
            output,
            "    published-crash interface {:?} routes {:?}",
            callable.checked_crash().interface(),
            callable.checked_crash().published()
        )?;
        for (label, flows) in [
            ("capability-flow", callable.capability_flows()),
            (
                "reachable-capability-flow",
                callable.reachable_capability_flows(),
            ),
        ] {
            for flow in flows {
                writeln!(
                    output,
                    "    {label} {:?} {}",
                    flow.kind(),
                    self.name(flow.capability())
                )?;
            }
        }
        for installation in callable.unresolved_installation_reaches() {
            writeln!(
                output,
                "    unresolved-installation {}",
                self.name(installation.requirement())
            )?;
            self.reach(
                output,
                "installation-upper-bound",
                Some(installation.upper_bound()),
                "unknown",
            )?;
        }
        contracts(output, callable.contracts())
    }

    pub(super) fn reach(
        &self,
        output: &mut Output,
        label: &str,
        identities: Option<&[PackageReviewNominalIdentity]>,
        absent: &str,
    ) -> fmt::Result {
        write!(output, "    {label} ")?;
        match identities {
            None => write!(output, "{absent}")?,
            Some([]) => write!(output, "[]")?,
            Some(identities) => {
                for (index, identity) in identities.iter().enumerate() {
                    if index != 0 {
                        write!(output, ", ")?;
                    }
                    write!(output, "{}", self.name(identity))?;
                }
            }
        }
        writeln!(output)
    }

    pub(super) fn invocations(
        &self,
        output: &mut Output,
        label: &str,
        invocations: Option<&[PackageReviewSynchronousInvocation]>,
        absent: &str,
    ) -> fmt::Result {
        write!(output, "    {label} ")?;
        match invocations {
            None => write!(output, "{absent}")?,
            Some([]) => write!(output, "[]")?,
            Some(invocations) => {
                for (index, invocation) in invocations.iter().enumerate() {
                    if index != 0 {
                        write!(output, ", ")?;
                    }
                    match invocation {
                        PackageReviewSynchronousInvocation::Parameter(ordinal) => {
                            write!(output, "parameter {ordinal}")?
                        }
                        PackageReviewSynchronousInvocation::Service(service) => {
                            write!(output, "{}", self.name(service))?
                        }
                    }
                }
            }
        }
        writeln!(output)
    }

    pub(super) fn method(
        &self,
        output: &mut Output,
        method: &PackagePolicyServiceMethod,
    ) -> fmt::Result {
        writeln!(
            output,
            "    method {:?} {} suspend {} block {}",
            method.name(),
            self.name(method.requirement()),
            method.may_suspend(),
            method.may_block()
        )?;
        self.reach(
            output,
            "service-authority-reach",
            Some(method.authority().service_reach()),
            "unknown",
        )?;
        self.invocations(
            output,
            "service-authority-invocations",
            Some(method.authority().synchronous_invocations()),
            "unknown",
        )?;
        writeln!(
            output,
            "    entry-claims {:?} result-claims {:?}; progress-premises {} (--details)",
            method.entry_claims(),
            method.result_claims(),
            method.authority().progress_premises().len()
        )
    }
}
