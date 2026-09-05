use crate::capture::behavior::canonical_checked_invocation_targets;
use crate::capture::semantics::facts::exactly_one;
use crate::capture::source::ProjectedNestedSourceLocation;
use crate::record::PackageReviewSourceLocationRole;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;

pub(crate) fn project_machine_invocation_source_locations(
    compilation: &CheckedCompilation,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    validate_machine_invocations(compilation, machine)?;
    let declarations = compilation.machine_invokes(machine);
    Ok(declarations
        .iter()
        .map(|declaration| ProjectedNestedSourceLocation {
            source_span: declaration.source_span,
            role: PackageReviewSourceLocationRole::SynchronousInvocation,
        })
        .collect())
}

pub(crate) fn validate_machine_invocations(
    compilation: &CheckedCompilation,
    machine: &typed_trees::machine::Machine,
) -> Result<(), Vec<Diagnostic>> {
    let declarations = compilation.machine_invokes(machine);
    let declared = flow_effects::declared_machine_invocations(compilation, machine);
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

    Ok(())
}

pub(crate) fn project_signature_invocation_source_locations(
    compilation: &CheckedCompilation,
    signature: &typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let declarations = compilation.state_signature_invokes(signature);
    let targets = flow_effects::declared_signature_invocations(compilation, signature);
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
