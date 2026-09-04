use crate::capture::semantics::facts::exactly_one;
use crate::capture::source::ProjectedNestedSourceLocation;
use crate::capture::source::locations::canonical_source_span_location;
use crate::record::PackageReviewSourceLocationRole;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_machine_operational_source_locations(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = project_operational_keyword_locations(
        compilation,
        machine.name.as_str(),
        "suspends",
        machine.suspends,
        &machine.suspends_keyword_source_spans,
        PackageReviewSourceLocationRole::Suspension,
    )?;
    locations.extend(project_operational_keyword_locations(
        compilation,
        machine.name.as_str(),
        "blocks",
        machine.blocks,
        &machine.blocks_keyword_source_spans,
        PackageReviewSourceLocationRole::Blocking,
    )?);

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
        || machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody;
    let expected_suspension = if publishes || machine.suspends {
        psi_language_semantics::SuspensionInterface::PublishedMaySuspend(machine.suspends)
    } else {
        psi_language_semantics::SuspensionInterface::InternalInferred
    };
    let expected_blocking = if publishes || machine.blocks {
        psi_language_semantics::BlockingInterface::PublishedMayBlock(machine.blocks)
    } else {
        psi_language_semantics::BlockingInterface::InternalInferred
    };
    if suspension.interface != expected_suspension || blocking.interface != expected_blocking {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored operational custody does not equal its exact checked interfaces",
            machine.name
        ))]);
    }
    Ok(locations)
}

pub(crate) fn project_signature_operational_source_locations(
    compilation: &CheckedCompilation,
    owner: SymbolHandle,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = project_operational_keyword_locations(
        compilation,
        signature.name.as_str(),
        "suspends",
        signature.suspends,
        &signature.suspends_keyword_source_spans,
        PackageReviewSourceLocationRole::Suspension,
    )?;
    locations.extend(project_operational_keyword_locations(
        compilation,
        signature.name.as_str(),
        "blocks",
        signature.blocks,
        &signature.blocks_keyword_source_spans,
        PackageReviewSourceLocationRole::Blocking,
    )?);
    let checked = exactly_one(
        compilation
            .facts
            .contract_plans
            .crash_capsules
            .iter()
            .filter(|capsule| {
                capsule.target_machine() == owner && capsule.target_state() == signature.symbol
            }),
        signature.name.as_str(),
        "signature contract capsule",
    )?;
    if checked.published_may_suspend() != signature.suspends
        || checked.published_may_block() != signature.blocks
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` authored operational custody does not equal its exact checked contract capsule",
            signature.name
        ))]);
    }
    Ok(locations)
}

fn project_operational_keyword_locations(
    compilation: &CheckedCompilation,
    owner_name: &str,
    clause: &str,
    authored: bool,
    source_spans: &[psi_source::SourceSpan],
    role: PackageReviewSourceLocationRole,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    if authored == source_spans.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has contradictory authored `{clause}` source custody"
        ))]);
    }
    source_spans
        .iter()
        .copied()
        .map(|source_span| {
            canonical_source_span_location(compilation, source_span, role)?;
            Ok(ProjectedNestedSourceLocation { source_span, role })
        })
        .collect()
}
