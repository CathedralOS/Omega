//! Project authored target scope through the exact family-member association.

use crate::capture::PackageReviewInput;
use crate::record::PackagePolicyTargetContext;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
) -> Result<PackagePolicyTargetContext, Vec<Diagnostic>> {
    let context = member_context(&compilation.typed, machine)?;
    // Review accepts a separately supplied semantic candidate. Agreement of
    // its callable and supply rows alone cannot authenticate a deleted family
    // association: the selected member has no mutable target marker left.
    if context != member_context(&compilation.custody.typed, machine)? {
        return Err(rejected(
            "authored target context differs from checked source custody",
        ));
    }
    Ok(context)
}

fn member_context(
    typed: &TypedTrees,
    machine: &Machine,
) -> Result<PackagePolicyTargetContext, Vec<Diagnostic>> {
    // Selection changes Machine::target, but never the member association.
    // Do not infer portability from the selected member's cleared marker.
    Ok(
        match typed
            .authored_machine_target(machine.symbol)
            .map_err(rejected)?
        {
            Some(target) => PackagePolicyTargetContext::TargetMember {
                target: target.to_owned(),
            },
            None => PackagePolicyTargetContext::TargetIndependent,
        },
    )
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "package target context rejects {reason}"
    ))]
}
