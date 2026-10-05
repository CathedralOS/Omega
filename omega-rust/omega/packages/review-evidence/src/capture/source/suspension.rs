use crate::capture::PackageReviewInput;
use crate::capture::source::locations::canonical_source_span_location;
use crate::record::PackageReviewSourceLocationRole;
use diagnostics::Diagnostic;

pub(crate) fn validate_machine_operational(
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
    validate_machine_operational_interfaces(compilation, machine)
}

fn validate_machine_operational_interfaces(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
) -> Result<(), Vec<Diagnostic>> {
    let suspension = compilation
        .facts
        .suspensions
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{}` has no exact suspension fact",
                machine.name
            ))]
        })?;
    let blocking = compilation
        .facts
        .blocking
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{}` has no exact blocking fact",
                machine.name
            ))]
        })?;
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
