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
//! A component whose realization rows match but whose module still retains
//! unresolved installation-bound requirement rows also rejects:
//! `realizes_selected_plan` reports those rows as their own mismatch, since
//! the description publishes each retained bound as a service bound
//! installation still owes, never folded into the concrete reach the
//! `ServiceCeiling` rows carry. A successful join establishes that the
//! realization exists inside the verified subject; it grants no callable
//! authority and discharges no installation obligation.

use component_description::{IndependentRealizationMismatch, VerifiedComponent};
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
    /// and a matching component still retaining unresolved installation-bound
    /// rows reject separately; none of them falls back to a fused edge.
    pub(crate) fn realize(&mut self, plan: &ProviderPlan) -> Result<(), diagnostics::Diagnostic> {
        if self.components.is_empty() {
            return Err(diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` retains independent composition, but no verified component description was supplied to realize it; refusing to treat the edge as fused",
                plan.name,
            )));
        }
        let mut realizing = Vec::new();
        let mut unresolved = Vec::new();
        let mut mismatches = Vec::new();
        for (index, component) in self.components.iter().enumerate() {
            match component.realizes_selected_plan(plan) {
                Ok(()) => realizing.push(index),
                Err(IndependentRealizationMismatch::UnresolvedInstallationRows {
                    requirement_identities,
                }) => unresolved.push((index, requirement_identities)),
                Err(mismatch) => mismatches.push(format!(
                    "component {}: {mismatch}",
                    description_identity(component)
                )),
            }
        }
        match realizing.as_slice() {
            [index] => {
                self.realized[*index] = true;
                Ok(())
            }
            [] if !unresolved.is_empty() => {
                let detail = unresolved
                    .iter()
                    .map(|(index, identities)| {
                        format!(
                            "verified component {} realizes it but retains {} unresolved installation-bound requirement row(s) ({})",
                            description_identity(&self.components[*index]),
                            identities.len(),
                            identities.join(", "),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                let others = if mismatches.is_empty() {
                    String::new()
                } else {
                    format!("; other components: {}", mismatches.join("; "))
                };
                Err(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` retains independent composition, but {detail}{others}; the component's published service bounds are obligations installation still owes, not resolved reach, refusing to treat the edge as fused",
                    plan.name,
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
