//! Closed source applications served by an unchanged selected foreign binding.
//!
//! These are requirement coordinates, not checked-body specializations. Their
//! producer retains the complete closed source tuple; the selected plan retains
//! the evaluated locator and calling contract. Neither this join nor its lookup
//! grants execution, same-stack custody, or receiving permission.

use super::{SelectedProviderPlanFacts, selected_closure_report_fingerprint};
use crate::provider_plan::{ProviderBinding, ProviderPlanDigest};
use terminal_psi::{TerminalIdentityIndex, TerminalIdentityTable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExternalRequirementApplication {
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub selected_requirement_identity: TerminalIdentityIndex,
    /// Exact closed identity, including outer and local static arguments.
    pub requirement_identity: TerminalIdentityIndex,
}

impl SelectedProviderPlanFacts {
    pub fn external_requirement_applications(&self) -> &[SelectedExternalRequirementApplication] {
        &self.external_requirement_applications
    }

    pub fn external_requirement_application(
        &self,
        requirement: &str,
    ) -> Option<&SelectedExternalRequirementApplication> {
        self.external_requirement_applications
            .iter()
            .find(|row| self.identities.text(row.requirement_identity) == Some(requirement))
    }

    /// Attach source-authenticated coordinates without rewriting selected rows.
    /// Reject cross-plan aliases and collisions with checked realizations.
    pub fn with_external_requirement_applications(
        mut self,
        mut rows: Vec<SelectedExternalRequirementApplication>,
        row_identities: &TerminalIdentityTable,
    ) -> Result<Self, String> {
        let mut identities = self.identities.clone().into_builder();
        for row in &mut rows {
            row.selected_requirement_identity = identities.intern(
                row_identities
                    .text(row.selected_requirement_identity)
                    .unwrap_or_default(),
            );
            row.requirement_identity = identities.intern(
                row_identities
                    .text(row.requirement_identity)
                    .unwrap_or_default(),
            );
        }
        self.identities = identities.build();
        rows.sort_by_cached_key(|row| {
            self.identities
                .text(row.requirement_identity)
                .unwrap_or_default()
                .to_owned()
        });
        for pair in rows.windows(2) {
            if pair[0].requirement_identity == pair[1].requirement_identity {
                return Err(
                    "closed external requirement has multiple selected applications".into(),
                );
            }
        }
        for row in &rows {
            if row.selected_requirement_identity.is_absent()
                || row.requirement_identity.is_absent()
                || row.selected_requirement_identity == row.requirement_identity
            {
                return Err(
                    "closed external requirement lost its distinct source identities".into(),
                );
            }
            let plan = self
                .plan_by_report_fingerprint(row.provider_plan_report_identity)
                .ok_or("closed external requirement names an unselected plan")?;
            if plan.identity_digest() != row.provider_plan_digest {
                return Err("closed external requirement substituted its selected plan".into());
            }
            let mut matching = plan.rows.iter().filter(|selected| {
                Some(plan.identities.text(selected.requirement_identity))
                    == self.identities.text(row.selected_requirement_identity)
            });
            let selected = matching
                .next()
                .ok_or("closed external requirement lost its selected row")?;
            if matching.next().is_some()
                || !matches!(selected.binding, ProviderBinding::Import { .. })
            {
                return Err(
                    "closed external requirement requires exactly one selected Import row".into(),
                );
            }
            let requirement = self
                .identities
                .text(row.requirement_identity)
                .unwrap_or_default();
            if self.specialized_requirement_row(requirement).is_some()
                || self.family_realization_row(requirement).is_some()
                || self.plans.iter().any(|candidate| {
                    candidate.rows.iter().any(|selected| {
                        candidate.identities.text(selected.requirement_identity) == requirement
                    })
                })
            {
                return Err(
                    "closed external requirement shadows another selected coordinate".into(),
                );
            }
        }
        self.external_requirement_applications = rows;
        self.report_fingerprint = selected_closure_report_fingerprint(
            &self.plans,
            &self.installation_reach_resolutions,
            &self.specialized_requirement_rows,
            &self.family_realization_rows,
            &self.external_requirement_applications,
            &self.identities,
        );
        Ok(self)
    }
}
