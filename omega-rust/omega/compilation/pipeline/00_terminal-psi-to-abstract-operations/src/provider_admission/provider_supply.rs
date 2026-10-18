//! Provider supply for the boundary requirements a program entry reaches.
//!
//! Selection chooses among declared candidates, and a missing candidate
//! rejects (`wiki/spec/build/provider_selection.md`).
//! A requirement the entry calls directly, `Requirement::operation(..)`, has
//! no source carrier that could name its boundary early, so its demand is read
//! here from this target's realized plan. Psi's invocation facts union every
//! member of a target family; the realized plan reaches only the member this
//! target selected, so another target's raw family is never demanded.
//!
//! The walk is the closure review's own (`reachability`). This check names the
//! entry's unselected requirements before settlement reports the same omission
//! as a missing import, mechanism or provider row. A selected provider's
//! nested calls are closed by provider planning; the review still refuses any
//! reachable requirement without a row.

use ::diagnostics::Diagnostic;

use super::diagnostics::realization_error;
use super::terminal_authority_review::reachability::AuthorityGraph;

pub(super) fn require_reached_providers(
    input: &crate::VerifiedPsiOptimizationInput,
    target_profile: target::TargetProfile,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let plan = input.plan();
    let reached = AuthorityGraph::new(input, target_profile)
        .and_then(|graph| graph.reachable_authority_edges(plan.entry))
        .map_err(|error| realization_error("provider supply", error))?;
    let diagnostics = reached
        .boundaries
        .iter()
        .filter_map(|boundary| {
            // An undeclared boundary is the closure review's refusal.
            let declaration = plan
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == *boundary)?;
            let requirement = plan.identities.text(declaration.identity)?;
            (selected
                .selected_requirement_row(requirement)
                .is_none()
                && !selected.assumed_bindings().iter().any(|row| row.required == requirement)
            ).then(|| {
                    Diagnostic::error(format!(
                        "no provider is selected for boundary requirement `{}`, which the program entry reaches on target `{}`",
                        requirement_path(requirement),
                        target_profile.target_name(),
                    ))
                })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The authored path inside a canonical callable identity, for this
/// diagnostic only: `named-callable(path(Gui::window_create),parameters(..))`
/// renders as `Gui::window_create`. No join reads it; an identity without
/// that head renders whole.
fn requirement_path(identity: &str) -> &str {
    const HEAD: &str = "named-callable(path(";
    let Some(start) = identity.find(HEAD).map(|index| index + HEAD.len()) else {
        return identity;
    };
    let mut depth = 0usize;
    for (offset, character) in identity[start..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' if depth == 0 => return &identity[start..start + offset],
            ')' => depth -= 1,
            _ => {}
        }
    }
    identity
}

#[cfg(test)]
mod tests {
    use super::requirement_path;

    #[test]
    fn requirement_path_names_the_authored_callable() {
        assert_eq!(
            requirement_path(
                "named-callable(path(Acknowledgement::ack),parameters(parameter\\(x\\)),result-dispatch())"
            ),
            "Acknowledgement::ack"
        );
        assert_eq!(
            requirement_path(
                "closed-requirement(template(named-callable(path(Parent::read),parameters())))"
            ),
            "Parent::read"
        );
        assert_eq!(requirement_path("opaque"), "opaque");
    }
}
