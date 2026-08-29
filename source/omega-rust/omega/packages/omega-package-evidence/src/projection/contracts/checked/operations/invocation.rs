use crate::evidence::PackageReviewSourceLocationRole;
use crate::evidence::package::ProjectedNestedSourceLocation;
use crate::projection::semantics::facts::exactly_one;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

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
