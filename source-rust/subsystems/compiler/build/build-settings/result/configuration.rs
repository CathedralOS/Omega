//! Normalized Build choices, their accepted source and the observations of this execution.
use behavior_exclusion_requests::AuthoredBehaviorExclusion;
use checked_program_to_psi::provider_choice_inputs::{NativeBuildSupply, ProviderSelection};
use normalized_build_settings::EvaluatedBuildSettings;
use source_declarations::source_selections::{
    AssumedBindingContract, RootBinding, ServiceAuditRegistration, TestGroupRegistration,
    WireCompatibilityDemand,
};
use symbols::root_grants::AuthoredRootGrant;
use typed_trees::source_state::opaque_representations::OpaqueRepresentationSelection;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildConfig {
    pub settings: EvaluatedBuildSettings,
    pub grants: Vec<AuthoredRootGrant>,
    pub provider_selections: Vec<ProviderSelection>,
    pub default_provider_selections: Vec<ProviderSelection>,
    pub native_bindings: Vec<NativeBuildSupply>,
    pub native_binding_evaluation:
        Option<std::sync::Arc<evaluation_receipts::NativeBuildEvaluation>>,
    pub assumed_bindings: Vec<AssumedBindingContract>,
    pub opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    pub wire_compatibility_demands: Vec<WireCompatibilityDemand>,
    pub root_bindings: Vec<RootBinding>,
    pub behavior_exclusions: Vec<AuthoredBehaviorExclusion>,
    pub service_audits: Vec<ServiceAuditRegistration>,
    // Registered test groups (wiki/spec/build/testing.md). They select test
    // roots beside the application; they never become application roots.
    pub test_groups: Vec<TestGroupRegistration>,
}

#[derive(Clone)]
pub struct ComputedBuildConfig {
    /// The Build judgment whose produced invocation returned this configuration.
    /// Settings and observation digests cannot reconstruct its checked contracts.
    pub accepted_build: Option<std::sync::Arc<checked_program_to_psi::AcceptedTerminalSource>>,
    pub config: BuildConfig,
    pub root_binding_requests: Vec<source_declarations::declaration_selection::ExecutedRootBinding>,
    pub declared_product_selections:
        Vec<std::sync::Arc<source_declarations::declaration_selection::ProductDeclarationRequest>>,
    pub product_selection_requests:
        Vec<source_declarations::declaration_selection::ExecutedProductSelection>,
    pub native_formations: Vec<source_declarations::declaration_selection::NativeBindingFormation>,
    pub native_binding_requests:
        Vec<source_declarations::declaration_selection::ExecutedNativeBinding>,
    pub behavior_exclusion_requests:
        Vec<source_declarations::declaration_selection::ExecutedEnumExclusion>,
    pub project_declaration: Option<build_declarations::BuildDeclaration>,
    pub optimization_report_request: optimization_requests::OptimizationReportRequest,
    pub evaluation_usage: Option<evaluation_contracts::BuildEvaluationUsage>,
    pub observation_summary: Option<package_compilation::BuildObservationSummary>,
    pub selected_build_machine_symbol: Option<symbols::SymbolHandle>,
    pub generated_sources: Vec<build_output::PackageGeneratedSource>,
}

// Retaining a judgment changes custody, not the meaning of Build's output.
// Fresh equivalent executions already compare by their values and observations;
// an Arc address must not become another output/cache identity.
impl PartialEq for ComputedBuildConfig {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            accepted_build: _,
            config,
            root_binding_requests,
            declared_product_selections,
            product_selection_requests,
            native_formations,
            native_binding_requests,
            behavior_exclusion_requests,
            project_declaration,
            optimization_report_request,
            evaluation_usage,
            observation_summary,
            selected_build_machine_symbol,
            generated_sources,
        } = self;
        config == &other.config
            && root_binding_requests == &other.root_binding_requests
            && declared_product_selections == &other.declared_product_selections
            && product_selection_requests == &other.product_selection_requests
            && native_formations == &other.native_formations
            && native_binding_requests == &other.native_binding_requests
            && behavior_exclusion_requests == &other.behavior_exclusion_requests
            && project_declaration == &other.project_declaration
            && optimization_report_request == &other.optimization_report_request
            && evaluation_usage == &other.evaluation_usage
            && observation_summary == &other.observation_summary
            && selected_build_machine_symbol == &other.selected_build_machine_symbol
            && generated_sources == &other.generated_sources
    }
}

impl Eq for ComputedBuildConfig {}

impl std::fmt::Debug for ComputedBuildConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // A diagnostic describes the configuration, not every table and body
        // in the retained source judgment.
        formatter
            .debug_struct("ComputedBuildConfig")
            .field("accepted_build", &self.accepted_build.is_some())
            .field("config", &self.config)
            .field("root_binding_requests", &self.root_binding_requests)
            .field(
                "declared_product_selections",
                &self.declared_product_selections,
            )
            .field(
                "product_selection_requests",
                &self.product_selection_requests,
            )
            .field("native_formations", &self.native_formations)
            .field("native_binding_requests", &self.native_binding_requests)
            .field(
                "behavior_exclusion_requests",
                &self.behavior_exclusion_requests,
            )
            .field("project_declaration", &self.project_declaration)
            .field(
                "optimization_report_request",
                &self.optimization_report_request,
            )
            .field("evaluation_usage", &self.evaluation_usage)
            .field("observation_summary", &self.observation_summary)
            .field(
                "selected_build_machine_symbol",
                &self.selected_build_machine_symbol,
            )
            .field("generated_sources", &self.generated_sources)
            .finish()
    }
}

impl ComputedBuildConfig {
    pub fn selected_machine_identity(
        &self,
        typed: &typed_trees::source_state::TypedTrees,
    ) -> Result<Option<String>, Vec<diagnostics::Diagnostic>> {
        self.selected_build_machine_symbol
            .map(|selected| {
                let machine = typed.machine(selected).ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "selected build machine disappeared before final checking",
                    )]
                })?;
                typed
                    .machine_callable_identity_text(machine)
                    .ok_or_else(|| {
                        vec![diagnostics::Diagnostic::error(
                            "selected build machine has no canonical callable identity",
                        )]
                    })
            })
            .transpose()
    }
}
