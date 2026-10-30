//! Complete selected-closure candidates for compiler-private recovery.
//! Source-free native projection and receiving policy still admit actual use.
use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanCandidate {
    pub plans: Vec<ProviderPlan>,
    pub assumed_bindings: Vec<AssumedBindingContractSelection>,
    pub report_fingerprint: u64,
    pub installation_reach_resolutions: Vec<InstallationReachResolution>,
    pub specialized_requirement_rows: Vec<SelectedRequirementSpecialization>,
    pub external_requirement_applications: Vec<SelectedExternalRequirementApplication>,
    pub toolchain_settled_identities: BTreeSet<u64>,
    pub identities: terminal_psi::TerminalIdentityTable,
}

impl SelectedProviderPlanFacts {
    pub fn recovery_candidate(&self) -> SelectedProviderPlanCandidate {
        let Self {
            plans,
            assumed_bindings,
            report_fingerprint,
            installation_reach_resolutions,
            specialized_requirement_rows,
            external_requirement_applications,
            toolchain_settled_identities,
            identities,
        } = self;
        SelectedProviderPlanCandidate {
            plans: plans.clone(),
            assumed_bindings: assumed_bindings.clone(),
            report_fingerprint: *report_fingerprint,
            installation_reach_resolutions: installation_reach_resolutions.clone(),
            specialized_requirement_rows: specialized_requirement_rows.clone(),
            external_requirement_applications: external_requirement_applications.clone(),
            toolchain_settled_identities: toolchain_settled_identities.clone(),
            identities: identities.clone(),
        }
    }
}

impl SelectedProviderPlanCandidate {
    pub fn validate(self) -> Result<SelectedProviderPlanFacts, String> {
        let mut ordinary = Vec::new();
        let mut toolchain = Vec::new();
        for plan in &self.plans {
            if self
                .toolchain_settled_identities
                .contains(&plan.report_fingerprint())
            {
                toolchain.push(plan.clone());
            } else {
                ordinary.push(plan.clone());
            }
        }
        let mut recovered = SelectedProviderPlanFacts::from_selected_plans(ordinary)?;
        for plan in toolchain {
            recovered = recovered.with_toolchain_settled_plan(plan)?;
        }
        if recovered.plans != self.plans
            || recovered.toolchain_settled_identities != self.toolchain_settled_identities
            || recovered
                .identities
                .entries()
                .iter()
                .any(|identity| self.identities.index_of(identity).is_none())
        {
            return Err(
                "recovered selected provider roster changed order, membership or identity custody"
                    .to_owned(),
            );
        }
        // Preserve exact table slots, including unused declarations. Builders
        // rejoin rows by their independently retained text in this same table.
        recovered.identities = self.identities.clone();
        recovered = recovered.with_assumed_bindings(self.assumed_bindings.clone());
        for row in &self.assumed_bindings {
            recovered.assumed_supplier_identity(&row.required)?;
        }
        recovered = recovered.with_installation_reach_resolutions(
            self.installation_reach_resolutions.clone(),
            &self.identities,
        )?;
        recovered = recovered.with_specialized_requirement_rows(
            self.specialized_requirement_rows.clone(),
            &self.identities,
        )?;
        recovered = recovered.with_external_requirement_applications(
            self.external_requirement_applications.clone(),
            &self.identities,
        )?;
        if recovered.recovery_candidate() != self {
            return Err(
                "recovered provider closure is not canonical or lost a retained field".to_owned(),
            );
        }
        Ok(recovered)
    }
}
