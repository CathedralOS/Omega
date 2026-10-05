use crate::capture::PackageReviewInput;
use crate::capture::behavior::canonical_checked_invocation_targets;
use crate::capture::semantics::facts::exactly_one;
use diagnostics::Diagnostic;

pub(crate) fn validate_machine_invocations(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
) -> Result<(), Vec<Diagnostic>> {
    let declarations = compilation.machine_invokes(machine);
    let declared = validation::declared_machine_invocations(compilation, machine);
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
