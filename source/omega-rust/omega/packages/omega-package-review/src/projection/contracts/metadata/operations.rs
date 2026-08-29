use crate::model::{PackageReviewSourceLocationRole, ProjectedNestedSourceLocation};
use crate::projection::evidence::canonical_source_span_location;
use crate::projection::exact_identity::exactly_one;
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

pub(crate) fn project_operational_keyword_locations(
    compilation: &CheckedCompilation,
    owner_name: &str,
    clause: &str,
    authored: bool,
    source_spans: &[psi_source::SourceSpan],
    role: PackageReviewSourceLocationRole,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    if authored != !source_spans.is_empty() {
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

pub(crate) fn project_machine_invocation_source_locations(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let declarations = compilation.machine_invokes(machine);
    let declared = psi_effects::declared_machine_invocations(compilation, machine);
    if declared.len() != declarations.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` has an unresolved, duplicate, or semantically aliased authored invokes target",
            machine.name,
        ))]);
    }
    let checked = exactly_one(
        compilation
            .facts
            .synchronous_invocations
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "synchronous-invocation",
    )?;
    if checked.published_targets != declared {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored invokes targets do not equal its exact checked published ceiling",
            machine.name,
        ))]);
    }
    let checked_published = canonical_checked_invocation_targets(compilation, &declared)?;
    let checked_inferred =
        canonical_checked_invocation_targets(compilation, &checked.checked_inferred_targets)?;
    if checked.plan.published != checked_published
        || checked.plan.checked_inferred != checked_inferred
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` has contradictory exact and rendered synchronous-invocation facts",
            machine.name,
        ))]);
    }

    Ok(declarations
        .iter()
        .map(|declaration| ProjectedNestedSourceLocation {
            source_span: declaration.source_span,
            role: PackageReviewSourceLocationRole::SynchronousInvocation,
        })
        .collect())
}

pub(crate) fn project_signature_invocation_source_locations(
    compilation: &CheckedCompilation,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let declarations = compilation.state_signature_invokes(signature);
    let targets = psi_effects::declared_signature_invocations(compilation, signature);
    if targets.len() != declarations.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` has an unresolved, duplicate, or semantically aliased authored invokes target",
            signature.name,
        ))]);
    }

    Ok(declarations
        .iter()
        .map(|declaration| ProjectedNestedSourceLocation {
            source_span: declaration.source_span,
            role: PackageReviewSourceLocationRole::SynchronousInvocation,
        })
        .collect())
}

pub(crate) fn canonical_checked_invocation_targets(
    compilation: &CheckedCompilation,
    targets: &[psi_effects::InvocationTarget],
) -> Result<Vec<String>, Vec<Diagnostic>> {
    let mut canonical = targets
        .iter()
        .map(|target| match target {
            psi_effects::InvocationTarget::Parameter(index) => Ok(format!("parameter:{index}")),
            psi_effects::InvocationTarget::Service(symbol) => {
                let matching = compilation
                    .traits()
                    .iter()
                    .filter(|definition| definition.symbol == *symbol)
                    .collect::<Vec<_>>();
                let [definition] = matching.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed synchronous invocation resolves service symbol {} to {} declarations; expected exactly one",
                        symbol.arena_index(),
                        matching.len(),
                    ))]);
                };
                if !definition.is_boundary {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed synchronous invocation resolves `{}` to a non-boundary trait",
                        definition.name,
                    ))]);
                }
                Ok(format!("service:{}", definition.name))
            }
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    canonical.sort();
    canonical.dedup();
    Ok(canonical)
}
