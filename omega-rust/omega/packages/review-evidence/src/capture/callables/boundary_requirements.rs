//! Top-level requirement associations retain machine overload identity.
//! Checked bodies are revalidated against their requirement. Native bindings
//! retain separate provider-supply findings. Neither association chooses a
//! provider: selected-provider policy independently replays that decision.

use super::super::semantics::declarations::top_level_requirement_identity;
use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;

pub(super) fn project_checked_requirement_realization(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    conformance: &typed_trees::machine::TraitConformance,
    requirement: &typed_trees::machine::Machine,
) -> Result<crate::record::PackageReviewRequirementRealization, Vec<Diagnostic>> {
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` realizes top-level requirement `{}` without one checked implementation body",
            machine.name, requirement.name,
        ))]);
    }
    if machine.is_public && !requirement.is_public {
        return Err(vec![Diagnostic::error(format!(
            "reviewed public callable `{}` exposes private top-level requirement `{}`",
            machine.name, requirement.name,
        ))]);
    }
    validation::revalidate_top_level_requirement_realization(
        compilation.checking_input,
        machine,
        requirement,
        conformance,
    )?;
    Ok(crate::record::PackageReviewRequirementRealization {
        requirement_identity: top_level_requirement_identity(compilation, requirement)?,
        alias: conformance
            .alias
            .as_ref()
            .map(|alias| alias.as_str().to_owned()),
    })
}
