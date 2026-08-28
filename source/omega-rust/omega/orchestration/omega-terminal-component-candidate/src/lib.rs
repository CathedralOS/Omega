#![forbid(unsafe_code)]

//! Authority-free handoff from Terminal native realization to component
//! deployment.
//!
//! The universal native artifact lives in `omega-terminal-native-artifact`.
//! This crate adds only component entry identity, the complete source-derived
//! selected provider-plan facts needed by deployment, and any build-bound
//! progress manifest.

pub use omega_terminal_native_artifact::{
    TerminalNativeArtifact, TerminalNativeArtifactParts, TerminalNativeProviderExecution,
    TerminalNativeSelectedProviderPlan,
};

pub type TerminalComponentProviderExecution = TerminalNativeProviderExecution;

#[derive(Debug)]
pub struct TerminalComponentCandidate {
    native_artifact: TerminalNativeArtifact,
    entry_machine: String,
    selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    component_progress: Option<omega_effects::ComponentProgressManifest>,
}

#[derive(Debug)]
pub struct TerminalComponentCandidateParts {
    pub native_artifact: TerminalNativeArtifact,
    pub entry_machine: String,
    pub selected_provider_plans: omega_effects::SelectedProviderPlanFacts,
    pub component_progress: Option<omega_effects::ComponentProgressManifest>,
}

impl TerminalComponentCandidate {
    /// Rejoin component policy to one already replayed native artifact.
    pub fn checked(parts: TerminalComponentCandidateParts) -> Result<Self, &'static str> {
        parts.native_artifact.validate()?;
        validate_selected_provider_closure(
            parts.native_artifact.selected_provider_closure_identity(),
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

    pub const fn terminal_artifact(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        self.native_artifact.terminal_artifact()
    }

    pub const fn object(&self) -> &omega_terminal_image_emission::TerminalObjectArtifact {
        self.native_artifact.object()
    }

    pub const fn image(&self) -> &omega_terminal_image_emission::TerminalExecutableImage {
        self.native_artifact.image()
    }

    pub const fn selected_provider_plans(&self) -> &omega_effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[TerminalNativeProviderExecution] {
        self.native_artifact.provider_executions()
    }

    pub const fn native_artifact(&self) -> &TerminalNativeArtifact {
        &self.native_artifact
    }

    pub const fn component_progress(&self) -> Option<&omega_effects::ComponentProgressManifest> {
        self.component_progress.as_ref()
    }

    pub fn into_parts(self) -> TerminalComponentCandidateParts {
        TerminalComponentCandidateParts {
            native_artifact: self.native_artifact,
            entry_machine: self.entry_machine,
            selected_provider_plans: self.selected_provider_plans,
            component_progress: self.component_progress,
        }
    }
}

fn validate_selected_provider_closure(
    native_closure_identity: u64,
    native_plans: &[TerminalNativeSelectedProviderPlan],
    selected: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), &'static str> {
    if selected.normalized_identity() != native_closure_identity {
        return Err(
            "component candidate selected provider closure identity disagrees with its native artifact",
        );
    }
    let mut projected = selected
        .plans()
        .iter()
        .map(|plan| {
            TerminalNativeSelectedProviderPlan::new(
                plan.identity_fingerprint(),
                plan.rows
                    .iter()
                    .map(|row| row.requirement_identity.clone())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    projected.sort_by_key(TerminalNativeSelectedProviderPlan::identity);
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
    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };
    use omega_effects::{
        ConcreteIndexedProviderApplication, IndexedProviderConcreteArgument,
        IndexedProviderRequirementSchema, ProviderAssertedIndexedApplicationCoverage,
        SelectedProviderPlanFacts,
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
                    calling_plan_fingerprint: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "apply".into(),
                requirement_identity: "IndexedRequirement::apply".into(),
                binding: ProviderBinding::CheckedAdapter {
                    machine_identity: "IndexedProvider::apply".into(),
                    machine_package_identity: None,
                },
            }],
            origin_package_identity: None,
            origin_package: "test".into(),
        }
    }

    fn schema() -> IndexedProviderRequirementSchema {
        IndexedProviderRequirementSchema::new("IndexedRequirement", None, 2)
            .expect("valid free indexed schema")
    }

    fn projected(plan: &ProviderPlan) -> Vec<TerminalNativeSelectedProviderPlan> {
        vec![TerminalNativeSelectedProviderPlan::new(
            plan.identity_fingerprint(),
            vec!["IndexedRequirement::apply".into()],
        )]
    }

    #[test]
    fn component_replay_binds_indexed_coverage_even_when_plan_projection_is_unchanged() {
        let plan = selected_plan();
        let plan_identity = plan.identity_fingerprint();
        let base = SelectedProviderPlanFacts::from_selected_plans(vec![plan.clone()])
            .expect("complete selected plan");
        let generic = base
            .clone()
            .with_indexed_provider_application_coverage(vec![
                ProviderAssertedIndexedApplicationCoverage::generic(plan_identity, schema())
                    .expect("generic structural coverage"),
            ])
            .expect("generic coverage attaches to the selected slot");
        let exact_application = ConcreteIndexedProviderApplication::new(
            schema(),
            vec![
                IndexedProviderConcreteArgument::new("Plan").unwrap(),
                IndexedProviderConcreteArgument::new("Value").unwrap(),
            ],
        )
        .expect("exact structural application");
        let exact = base
            .with_indexed_provider_application_coverage(vec![
                ProviderAssertedIndexedApplicationCoverage::exact_family(
                    plan_identity,
                    schema(),
                    vec![exact_application],
                )
                .expect("exact structural coverage"),
            ])
            .expect("exact coverage attaches to the same selected slot");
        let native_plans = projected(&plan);

        validate_selected_provider_closure(generic.normalized_identity(), &native_plans, &generic)
            .expect("exact selected closure identity replays");
        assert_eq!(
            validate_selected_provider_closure(
                generic.normalized_identity(),
                &native_plans,
                &exact,
            ),
            Err(
                "component candidate selected provider closure identity disagrees with its native artifact"
            )
        );
    }
}
