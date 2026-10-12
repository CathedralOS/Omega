//! Complete applied requirements retain the exact whole selected row authority.
//!
//! Binding mechanisms do not determine public application identity. Checked body
//! and target-family receipts remain separate execution associations. This join
//! only closes the source producer's complete requirement coordinate against the
//! immutable selected plan; consumers never infer it from an ABI or template name.

use super::{
    SelectedProviderPlanFacts, SelectedRequirementApplication, selected_closure_report_fingerprint,
};

impl SelectedProviderPlanFacts {
    pub fn with_requirement_applications(
        mut self,
        mut applications: Vec<SelectedRequirementApplication>,
        application_identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, String> {
        let mut identities = self.identities.clone().into_builder();
        for application in &mut applications {
            application.selected_requirement_identity = identities.intern(
                application_identities
                    .text(application.selected_requirement_identity)
                    .unwrap_or_default(),
            );
            application.requirement_identity = identities.intern(
                application_identities
                    .text(application.requirement_identity)
                    .unwrap_or_default(),
            );
        }
        self.identities = identities.build();
        applications.sort_by_cached_key(|application| {
            self.identities
                .text(application.requirement_identity)
                .unwrap_or_default()
                .to_owned()
        });
        for pair in applications.windows(2) {
            if pair[0].requirement_identity == pair[1].requirement_identity {
                return Err(
                    "applied requirement has competing selected row correspondences".into(),
                );
            }
        }
        for application in &applications {
            if application.selected_requirement_identity.is_absent()
                || application.requirement_identity.is_absent()
            {
                return Err("selected requirement application has an empty identity".into());
            }
            let plan = self
                .plan_by_report_fingerprint(application.provider_plan_report_identity)
                .ok_or("selected requirement application names an absent selected plan")?;
            if plan.identity_digest() != application.provider_plan_digest
                || plan
                    .rows
                    .iter()
                    .filter(|row| {
                        Some(plan.identity_text(row.requirement_identity))
                            == self
                                .identities
                                .text(application.selected_requirement_identity)
                    })
                    .count()
                    != 1
            {
                return Err(
                    "selected requirement application changed its exact selected row".into(),
                );
            }
            let requirement = self
                .identities
                .text(application.requirement_identity)
                .unwrap_or_default();
            if let Some(specialization) = self.specialized_requirement_row(requirement)
                && (specialization.provider_plan_report_identity
                    != application.provider_plan_report_identity
                    || specialization.selected_requirement_identity
                        != application.selected_requirement_identity)
            {
                return Err("selected requirement application changed its retained checked body correspondence".into());
            }
            if let Some(family) = self.family_realization_row(requirement)
                && (family.provider_plan_report_identity
                    != application.provider_plan_report_identity
                    || family.selected_requirement_identity
                        != application.selected_requirement_identity)
            {
                return Err("selected requirement application changed its retained target family correspondence".into());
            }
        }
        for retained in &self.requirement_applications {
            if let Some(application) = applications.iter().find(|application| {
                application.requirement_identity == retained.requirement_identity
            }) {
                if application != retained {
                    return Err("selected requirement application changed an existing original correspondence".into());
                }
            } else {
                applications.push(retained.clone());
            }
        }
        applications.sort_by_cached_key(|application| {
            self.identities
                .text(application.requirement_identity)
                .unwrap_or_default()
                .to_owned()
        });
        self.requirement_applications = applications;
        self.report_fingerprint = selected_closure_report_fingerprint(
            &self.plans,
            &self.installation_reach_resolutions,
            &self.specialized_requirement_rows,
            &self.requirement_applications,
            &self.family_realization_rows,
            &self.identities,
        );
        Ok(self)
    }

    pub fn requirement_application(
        &self,
        requirement: &str,
    ) -> Option<&SelectedRequirementApplication> {
        self.requirement_applications.iter().find(|application| {
            self.identities.text(application.requirement_identity) == Some(requirement)
        })
    }

    /// Original selected-row associations, issued in this closure's identity table.
    pub fn requirement_applications(&self) -> &[SelectedRequirementApplication] {
        &self.requirement_applications
    }
}
