use crate::capture::PackageReviewInput;
use crate::capture::semantics::facts::exactly_one;
use crate::capture::source::locations::canonical_source_span_location;
use crate::record::PackageReviewSourceLocationRole;
use diagnostics::Diagnostic;

pub(crate) fn validate_machine_operational(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
) -> Result<(), Vec<Diagnostic>> {
    validate_authored_keywords(compilation, machine)?;
    let (suspension, blocking) = operational_plans(compilation.program, machine)?;
    let Some(members) = super::family_interfaces::members(compilation, machine)? else {
        return validate_machine_operational_interfaces(machine, suspension, blocking);
    };
    let rejected = || {
        vec![Diagnostic::error(format!(
            "reviewed family `{}` operational interface differs from its exact complete member custody",
            machine.name,
        ))]
    };
    if machine.suspends || machine.blocks {
        return Err(rejected());
    }
    let mut may_suspend = false;
    let mut may_block = false;
    for member in members {
        validate_authored_keywords(compilation, member)?;
        let (member_suspension, member_blocking) = operational_plans(compilation.program, member)?;
        validate_machine_operational_interfaces(member, member_suspension, member_blocking)?;
        let original = operational_plans(compilation.custody, member)?;
        if (member_suspension, member_blocking) != original {
            return Err(rejected());
        }
        // The retained transitive rows already account for public contracts
        // and private inferred effects, independently for the two axes.
        may_suspend |= member_suspension.checked_may_suspend;
        may_block |= member_blocking.checked_may_block;
    }
    if suspension.interface
        != language_semantics::SuspensionInterface::PublishedMaySuspend(may_suspend)
        || blocking.interface != language_semantics::BlockingInterface::PublishedMayBlock(may_block)
        || suspension.checked_may_suspend != may_suspend
        || blocking.checked_may_block != may_block
    {
        return Err(rejected());
    }
    Ok(())
}

fn validate_authored_keywords(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
) -> Result<(), Vec<Diagnostic>> {
    validate_operational_keyword_custody(
        compilation,
        machine.name.as_str(),
        "suspends",
        machine.suspends,
        &machine.suspends_keyword_source_spans,
        PackageReviewSourceLocationRole::Suspension,
    )?;
    validate_operational_keyword_custody(
        compilation,
        machine.name.as_str(),
        "blocks",
        machine.blocks,
        &machine.blocks_keyword_source_spans,
        PackageReviewSourceLocationRole::Blocking,
    )?;
    Ok(())
}

fn operational_plans(
    program: &checked_trees::CheckedTrees,
    machine: &typed_trees::machine::Machine,
) -> Result<
    (
        language_semantics::SuspensionPlan,
        language_semantics::BlockingPlan,
    ),
    Vec<Diagnostic>,
> {
    let suspension = exactly_one(
        program
            .facts
            .suspensions
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "suspension",
    )?;
    let blocking = exactly_one(
        program
            .facts
            .blocking
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "blocking",
    )?;
    Ok((suspension.plan, blocking.plan))
}

fn validate_machine_operational_interfaces(
    machine: &typed_trees::machine::Machine,
    suspension: language_semantics::SuspensionPlan,
    blocking: language_semantics::BlockingPlan,
) -> Result<(), Vec<Diagnostic>> {
    let publishes = machine.is_public
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody;
    let expected_suspension = if publishes || machine.suspends {
        language_semantics::SuspensionInterface::PublishedMaySuspend(machine.suspends)
    } else {
        language_semantics::SuspensionInterface::InternalInferred
    };
    let expected_blocking = if publishes || machine.blocks {
        language_semantics::BlockingInterface::PublishedMayBlock(machine.blocks)
    } else {
        language_semantics::BlockingInterface::InternalInferred
    };
    if suspension.interface != expected_suspension || blocking.interface != expected_blocking {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored operational custody does not equal its exact checked interfaces",
            machine.name
        ))]);
    }
    Ok(())
}

fn validate_operational_keyword_custody(
    compilation: &PackageReviewInput<'_>,
    owner_name: &str,
    clause: &str,
    authored: bool,
    source_spans: &[source::SourceSpan],
    role: PackageReviewSourceLocationRole,
) -> Result<(), Vec<Diagnostic>> {
    if authored == source_spans.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has contradictory authored `{clause}` source custody"
        ))]);
    }
    for source_span in source_spans {
        canonical_source_span_location(compilation, *source_span, role)?;
    }
    Ok(())
}
