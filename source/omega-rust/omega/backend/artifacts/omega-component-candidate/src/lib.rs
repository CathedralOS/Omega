#![forbid(unsafe_code)]

//! Authority-free handoff from native realization to component
//! deployment.
//!
//! The universal native artifact lives in `omega-native-artifact`.
//! This crate adds the component entry label, the complete source-derived
//! selected provider-plan facts needed by deployment, any build-bound progress
//! manifest, and emitter-derived stack demand for the canonical object entry.
//! The stack row describes the internal body closure only: it grants no
//! provision, lease, installed-root admission, or external-entry headroom.

pub use omega_native_artifact::{
    NativeArtifact, NativeArtifactParts, NativeProviderExecution,
    NativeSelectedProviderClosureDigest, NativeSelectedProviderPlan,
};

pub type ComponentProviderExecution = NativeProviderExecution;

#[derive(Debug)]
pub struct ComponentCandidate {
    native_artifact: NativeArtifact,
    entry_machine: String,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    component_progress: Option<omega_effects::ComponentProgressManifest>,
    stack_demand: omega_image_emission::StackDemand,
}

#[derive(Debug)]
pub struct ComponentCandidateParts {
    pub native_artifact: NativeArtifact,
    pub entry_machine: String,
    pub selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    pub component_progress: Option<omega_effects::ComponentProgressManifest>,
    pub stack_demand: omega_image_emission::StackDemand,
}

impl ComponentCandidate {
    /// Rejoin component policy to one already replayed native artifact.
    pub fn checked(parts: ComponentCandidateParts) -> Result<Self, &'static str> {
        parts.native_artifact.validate()?;
        validate_terminal_component_stack_demand(&parts.native_artifact, &parts.stack_demand)?;
        validate_selected_provider_closure(
            parts
                .native_artifact
                .selected_provider_closure_report_identity(),
            parts.native_artifact.selected_provider_closure_digest(),
            parts.native_artifact.selected_provider_plans(),
            &parts.selected_provider_plans,
        )?;
        if parts
            .component_progress
            .as_ref()
            .is_some_and(|manifest| manifest.pending().is_empty())
        {
            return Err("component candidate retained an empty progress manifest");
        }
        Ok(Self {
            native_artifact: parts.native_artifact,
            entry_machine: parts.entry_machine,
            selected_provider_plans: parts.selected_provider_plans,
            component_progress: parts.component_progress,
            stack_demand: parts.stack_demand,
        })
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.native_artifact.target()
    }

    pub fn entry_machine(&self) -> &str {
        &self.entry_machine
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        self.native_artifact.semantic_bytes()
    }

    pub fn proof_bytes(&self) -> &[u8] {
        self.native_artifact.proof_bytes()
    }

    pub const fn artifact(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        self.native_artifact.psi_artifact()
    }

    pub const fn object(&self) -> &omega_image_emission::ObjectArtifact {
        self.native_artifact.object()
    }

    pub const fn image(&self) -> &omega_image_emission::ExecutableImage {
        self.native_artifact.image()
    }

    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[NativeProviderExecution] {
        self.native_artifact.provider_executions()
    }

    pub const fn native_artifact(&self) -> &NativeArtifact {
        &self.native_artifact
    }

    pub const fn component_progress(&self) -> Option<&omega_effects::ComponentProgressManifest> {
        self.component_progress.as_ref()
    }

    /// Exact target-specific internal call-graph stack demand for the selected
    /// component entry. This is artifact evidence only; it is not runtime
    /// provision, a stack lease, or installed-root admission.
    pub const fn stack_demand(&self) -> &omega_image_emission::StackDemand {
        &self.stack_demand
    }

    pub fn into_parts(self) -> ComponentCandidateParts {
        ComponentCandidateParts {
            native_artifact: self.native_artifact,
            entry_machine: self.entry_machine,
            selected_provider_plans: self.selected_provider_plans,
            component_progress: self.component_progress,
            stack_demand: self.stack_demand,
        }
    }
}

/// Independently rederive a component candidate's complete selected-entry
/// stack demand. Equality includes the full native target, so equal-shaped
/// ELF, Mach-O, and COFF closures cannot substitute for one another.
fn validate_terminal_component_stack_demand(
    native_artifact: &NativeArtifact,
    supplied: &omega_image_emission::StackDemand,
) -> Result<(), &'static str> {
    let expected = omega_image_emission::derive_stack_demand(
        native_artifact.object(),
        native_artifact.object().entry(),
    )
    .map_err(|_| "component candidate could not derive its selected-entry stack demand")?;
    if &expected != supplied {
        return Err(
            "component candidate stack demand disagrees with the exact selected-entry artifact closure",
        );
    }
    Ok(())
}

fn validate_selected_provider_closure(
    native_closure_report_identity: u64,
    native_closure_digest: NativeSelectedProviderClosureDigest,
    native_plans: &[NativeSelectedProviderPlan],
    selected: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), &'static str> {
    if selected.compatibility_report_identity() != native_closure_report_identity {
        return Err(
            "component candidate selected provider closure report identity disagrees with its native artifact",
        );
    }
    if selected.identity_digest().as_bytes() != native_closure_digest.as_bytes() {
        return Err(
            "component candidate selected provider closure digest disagrees with its native artifact",
        );
    }
    let mut projected = selected
        .plans()
        .iter()
        .map(|plan| {
            NativeSelectedProviderPlan::new(
                plan.report_fingerprint(),
                plan.rows
                    .iter()
                    .map(|row| row.requirement_identity.clone())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    projected.sort_by_key(NativeSelectedProviderPlan::report_identity);
    if projected != native_plans {
        return Err(
            "component candidate selected provider facts disagree with its native artifact",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_effects::SelectedProviderPlanFacts;
    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };

    fn selected_plan() -> ProviderPlan {
        ProviderPlan {
            name: "SelectedIndexedProvider".into(),
            provider_type: "IndexedProvider".into(),
            provider_type_package_identity: None,
            target: "test".into(),
            schema: ServiceSchema {
                trait_name: "IndexedRequirement".into(),
                trait_package_identity: None,
                methods: vec![ServiceMethod {
                    name: "apply".into(),
                    requirement_owner: "IndexedRequirement".into(),
                    requirement_owner_package_identity: None,
                    requirement_identity: "IndexedRequirement::apply".into(),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["IndexedRequirement".into()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "apply".into(),
                requirement_identity: "IndexedRequirement::apply".into(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CheckedAdapter {
                    machine_identity: "IndexedProvider::apply".into(),
                    machine_package_identity: None,
                },
            }],
            origin_package_identity: None,
            origin_package: "test".into(),
        }
    }

    fn projected(plan: &ProviderPlan) -> Vec<NativeSelectedProviderPlan> {
        vec![NativeSelectedProviderPlan::new(
            plan.report_fingerprint(),
            vec!["IndexedRequirement::apply".into()],
        )]
    }

    #[test]
    fn component_replay_rejects_compact_equal_provider_plan_substitution() {
        let original_plan = selected_plan();
        let mut substituted_plan = original_plan.clone();
        substituted_plan.schema.methods[0].requirement_owner = "OtherIndexedRequirement".into();
        assert_eq!(
            original_plan.report_fingerprint(),
            substituted_plan.report_fingerprint(),
            "the legacy compact plan identity omits the readable requirement owner"
        );

        let original = SelectedProviderPlanFacts::from_selected_plans(vec![original_plan.clone()])
            .expect("original selected closure");
        let substituted =
            SelectedProviderPlanFacts::from_selected_plans(vec![substituted_plan.clone()])
                .expect("compact-equal substituted closure");
        assert_eq!(
            original.compatibility_report_identity(),
            substituted.compatibility_report_identity(),
        );
        assert_ne!(original.identity_digest(), substituted.identity_digest());

        let native_plans = projected(&original_plan);
        assert_eq!(native_plans, projected(&substituted_plan));
        assert_eq!(
            validate_selected_provider_closure(
                original.compatibility_report_identity(),
                NativeSelectedProviderClosureDigest::from_digest(
                    *original.identity_digest().as_bytes(),
                ),
                &native_plans,
                &substituted,
            ),
            Err(
                "component candidate selected provider closure digest disagrees with its native artifact"
            )
        );
    }
}
