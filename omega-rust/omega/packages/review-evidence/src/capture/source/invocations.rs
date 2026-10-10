use crate::capture::PackageReviewInput;
use crate::capture::behavior::canonical_checked_invocation_targets;
use crate::capture::semantics::facts::exactly_one;
use diagnostics::Diagnostic;

pub(crate) fn validate_machine_invocations(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
) -> Result<(), Vec<Diagnostic>> {
    let declarations = compilation.machine_invokes(machine);
    let declared = validation::declared_machine_invocations(compilation.checking_input, machine);
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
    let expected = family_invocation_ceiling(compilation, machine)?.unwrap_or(declared);
    if checked.published_targets != expected {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored invokes targets do not equal its exact checked published ceiling",
            machine.name,
        ))]);
    }
    let checked_published = canonical_checked_invocation_targets(compilation, &expected)?;
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

/// A virtual family head has no authored invokes clause: its published ceiling
/// is the union of every retained member's effective checked ceiling. Source
/// review verifies that derivation against immutable compilation custody; it
/// neither selects a deployment nor runs semantic inference again.
fn family_invocation_ceiling(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
) -> Result<Option<Vec<flow_effects::InvocationTarget>>, Vec<Diagnostic>> {
    let Some(members) = super::family_interfaces::members(compilation, machine)? else {
        return Ok(None);
    };
    let rejected = |reason: &str| {
        vec![Diagnostic::error(format!(
            "reviewed family `{}` {reason}",
            machine.name
        ))]
    };
    if !compilation.machine_invokes(machine).is_empty() {
        return Err(rejected(
            "derived interface carries an authored invokes clause",
        ));
    }
    let mut union = Vec::new();
    for declaration in members {
        let declared =
            validation::declared_machine_invocations(compilation.checking_input, declaration);
        let checked = exactly_one(
            compilation
                .facts
                .synchronous_invocations
                .machines
                .iter()
                .filter(|fact| fact.machine == declaration.symbol),
            machine.name.as_str(),
            "family member synchronous-invocation",
        )?;
        let original = exactly_one(
            compilation
                .custody
                .facts
                .synchronous_invocations
                .machines
                .iter()
                .filter(|fact| fact.machine == declaration.symbol),
            machine.name.as_str(),
            "family member synchronous-invocation custody",
        )?;
        if declared.len() != compilation.machine_invokes(declaration).len()
            || checked.published_targets != declared
            || checked != original
        {
            return Err(rejected(
                "member authored invokes ceiling disagrees with its exact checked custody",
            ));
        }
        let effective = match checked.plan.interface {
            language_semantics::SynchronousInvocationInterface::PublishedCeiling => {
                &checked.published_targets
            }
            language_semantics::SynchronousInvocationInterface::InternalInferred => {
                &checked.checked_inferred_targets
            }
        };
        for target in effective {
            if !union.contains(target) {
                union.push(*target);
            }
        }
    }
    union.sort_by_key(|target| match target {
        flow_effects::InvocationTarget::Parameter(index) => (0_u8, *index),
        flow_effects::InvocationTarget::Service(symbol) => (1_u8, symbol.arena_index()),
    });
    let head = exactly_one(
        compilation
            .facts
            .synchronous_invocations
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "family head synchronous-invocation",
    )?;
    if head.plan.interface != language_semantics::SynchronousInvocationInterface::PublishedCeiling
        || head.checked_inferred_targets != union
    {
        return Err(rejected(
            "derived invocation interface differs from its complete member union",
        ));
    }
    Ok(Some(union))
}
