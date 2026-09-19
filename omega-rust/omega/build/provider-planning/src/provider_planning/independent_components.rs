//! Component-closure join for `Independent` provider selections.
//!
//! An `Independent` composition cut deploys the selected provider as its own
//! component. The build supplies the verified descriptions of those
//! components; this join requires every independently selected plan to be
//! realized by exactly one of them and every supplied component to realize
//! at least one such plan. It reads only `VerifiedComponent`, the
//! evidence-only carrier `component-description` verifies from the canonical
//! Terminal artifact, and joins through `realizes_selected_plan`, so no plan
//! row is ever compared against a hand-authored inventory.
//!
//! A realizing component whose module still exports unresolved
//! installation-bound requirement rows also rejects: the description folds
//! each declared dependency's conservative upper bound into one service
//! ceiling, so a retained row means the exported reach is a bound, not the
//! resolved service reach the selected row names. A successful join
//! establishes that the realization exists inside the verified subject; it
//! grants no callable authority and discharges no installation obligation.

use component_description::VerifiedComponent;
use effects::provider_plan::ProviderPlan;

/// The join state for one selection: which supplied components an
/// independently selected plan has claimed.
pub(crate) struct IndependentComponentJoin<'a> {
    components: &'a [VerifiedComponent],
    realized: Vec<bool>,
}

impl<'a> IndependentComponentJoin<'a> {
    pub(crate) fn new(components: &'a [VerifiedComponent]) -> Self {
        Self {
            components,
            realized: vec![false; components.len()],
        }
    }

    /// Join one independently selected plan to exactly one verified
    /// component. A missing component, an absent realization (with every
    /// component's distinct mismatch), more than one realizing component,
    /// and a realizing component still exporting unresolved installation-bound
    /// rows reject separately; none of them falls back to a fused edge.
    pub(crate) fn realize(&mut self, plan: &ProviderPlan) -> Result<(), diagnostics::Diagnostic> {
        if self.components.is_empty() {
            return Err(diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` retains independent composition, but no verified component description was supplied to realize it; refusing to treat the edge as fused",
                plan.name,
            )));
        }
        let mut realizing = Vec::new();
        let mut mismatches = Vec::new();
        for (index, component) in self.components.iter().enumerate() {
            match component.realizes_selected_plan(plan) {
                Ok(()) => realizing.push(index),
                Err(mismatch) => mismatches.push(format!(
                    "component {}: {mismatch}",
                    description_identity(component)
                )),
            }
        }
        match realizing.as_slice() {
            [index] => {
                self.realized[*index] = true;
                let component = &self.components[*index];
                let unresolved = &component
                    .module()
                    .root_service_reach
                    .installation_dependencies;
                if unresolved.is_empty() {
                    return Ok(());
                }
                let identities = unresolved
                    .iter()
                    .map(|dependency| dependency.requirement_identity.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` retains independent composition, but verified component {} realizing it exports {} unresolved installation-bound requirement row(s) ({}); the component's published service ceiling is a conservative bound, not resolved reach, refusing to treat the edge as fused",
                    plan.name,
                    description_identity(component),
                    unresolved.len(),
                    identities,
                )))
            }
            [] => Err(diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` retains independent composition, but no verified component realizes it ({}); refusing to treat the edge as fused",
                plan.name,
                mismatches.join("; "),
            ))),
            many => {
                let identities = many
                    .iter()
                    .map(|index| description_identity(&self.components[*index]))
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` retains independent composition, but {} verified components realize it ({identities}); exactly one component may deploy an independent edge, refusing to treat the edge as fused",
                    plan.name,
                    many.len(),
                )))
            }
        }
    }

    /// Every supplied component must have realized an independently
    /// selected plan: an unmatched description is not evidence for this
    /// build and must not be carried into its selected facts.
    pub(crate) fn finish(self) -> Vec<diagnostics::Diagnostic> {
        self.components
            .iter()
            .zip(self.realized)
            .filter(|(_, realized)| !realized)
            .map(|(component, _)| {
                diagnostics::Diagnostic::error(format!(
                    "verified component {} realizes no independently selected provider plan; every supplied component description must deploy one selected edge",
                    description_identity(component),
                ))
            })
            .collect()
    }
}

/// The description identity as lowercase hex: the coordinate a build author
/// can match against the published description bytes.
fn description_identity(component: &VerifiedComponent) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(64);
    for byte in component.description_identity() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
