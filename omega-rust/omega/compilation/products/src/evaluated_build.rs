// Checked Build evaluation result, normalized BuildConfig, and per-continuation
// BuildAdmission facts. `ComputedBuildConfig` retains the complete source-issued
// configuration and execution observations without depending on the evaluator's
// orchestration; `with_execution_observation` updates only the consuming
// occurrence's fresh invocation observations while preserving dependency
// native-binding evaluations and completed source requirements.

use build_time_evaluation::WireCompatibilityDemand;
use effects::AuthoredBehaviorExclusion;
use package_compilation::build_execution::build_value::EvaluatedBuildSettings;
use provider_planning::ProviderSelection;
use provider_planning::program_entry::RootBinding;
use typed_trees::opaque_representations::OpaqueRepresentationSelection;

#[derive(Clone)]
pub struct EvaluatedBuild {
    pub configuration: std::sync::Arc<ComputedBuildConfig>,
    pub restricted_build_requests:
        Vec<package_compilation::build_execution::RestrictedBuildRequest>,
    pub base_source_consumption_commitment:
        Option<package_compilation::PackageSourceConsumptionCommitment>,
    pub constants: BuildConstants,
    /// Costs of this Build context, not materialization evidence for Product.
    pub source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
}

/// What the Build admission's constants owe a Product frontier typed after
/// Build ran. That frontier holds the Build source again with its constants
/// unsettled, so whatever the admission evaluated has to be settled there
/// too. Both fields are read from the admission itself: the fold from what
/// it did, the names from the module it accepted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildConstants {
    /// The admission folded a constant under Build's own provider plans.
    pub folded_under_providers: bool,
    /// The constants the accepted Build module names
    /// (`AcceptedTerminalSource::constants_read`), each with its declaration
    /// and the value the judged source recorded for it.
    pub read: Vec<psi_compiler::ConstantRead>,
}

pub use build_time_evaluation::product_declarations::ServiceAuditRegistration;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildConfig {
    pub settings: EvaluatedBuildSettings,
    pub grants: Vec<trust_model::AuthoredRootGrant>,
    pub provider_selections: Vec<ProviderSelection>,
    pub default_provider_selections: Vec<ProviderSelection>,
    pub native_bindings: Vec<provider_planning::native_bindings::NativeBuildSupply>,
    pub native_binding_evaluation:
        Option<std::sync::Arc<provider_planning::native_bindings::NativeBuildEvaluation>>,
    pub assumed_bindings: Vec<build_time_evaluation::assumed_bindings::AssumedBindingContract>,
    pub opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    pub wire_compatibility_demands: Vec<WireCompatibilityDemand>,
    pub root_bindings: Vec<RootBinding>,
    pub behavior_exclusions: Vec<AuthoredBehaviorExclusion>,
    pub service_audits: Vec<ServiceAuditRegistration>,
    // Registered test groups (wiki/spec/build/testing.md). They select test
    // roots beside the application; they never become application roots.
    pub test_groups: Vec<build_time_evaluation::test_groups::TestGroupRegistration>,
}

pub struct ProviderDeployment {
    pub roots: Option<Vec<(String, symbols::SymbolHandle)>>, // None = reusable Source checking; Some(empty) = provider-only deployment
}

impl BuildConfig {
    pub fn provider_deployment(&self) -> Result<ProviderDeployment, Vec<diagnostics::Diagnostic>> {
        let deployed = !self.root_bindings.is_empty()
            || !self.provider_selections.is_empty()
            || self.default_provider_selections.iter().any(|selection| {
                selection
                    .origin
                    .activation()
                    .source()
                    .package_identity
                    .is_some()
            });
        let roots = self
            .root_bindings
            .iter()
            .map(|binding| {
                let (owner, _) = binding.slot.rsplit_once("::").ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "Build application root lost its exact target slot",
                    )]
                })?;
                let profile = target::TargetProfile::from_root_slot_owner(owner)
                    .map_err(|error| vec![error])?;
                Ok((
                    profile.target_name().to_owned(),
                    binding.implementation_symbol,
                ))
            })
            .collect::<Result<Vec<_>, Vec<diagnostics::Diagnostic>>>()?;
        Ok(ProviderDeployment {
            roots: deployed.then_some(roots),
        })
    }

    pub(crate) fn same_source_choices(&self, other: &Self) -> bool {
        let Self {
            settings,
            grants,
            provider_selections,
            default_provider_selections,
            native_bindings,
            native_binding_evaluation,
            assumed_bindings,
            opaque_representation_selections,
            wire_compatibility_demands,
            root_bindings,
            behavior_exclusions,
            service_audits,
            test_groups,
        } = self;
        settings == &other.settings
            && grants == &other.grants
            && provider_selections == &other.provider_selections
            && default_provider_selections == &other.default_provider_selections
            && native_binding_evaluation
                .as_ref()
                .map(|evaluation| evaluation.terminal_code)
                == other
                    .native_binding_evaluation
                    .as_ref()
                    .map(|evaluation| evaluation.terminal_code)
            && native_bindings.len() == other.native_bindings.len()
            && native_bindings
                .iter()
                .zip(&other.native_bindings)
                .all(|(left, right)| {
                    left.selection == right.selection
                        && left.evaluation.terminal_code == right.evaluation.terminal_code
                })
            && assumed_bindings == &other.assumed_bindings
            && opaque_representation_selections == &other.opaque_representation_selections
            && wire_compatibility_demands == &other.wire_compatibility_demands
            && root_bindings == &other.root_bindings
            && behavior_exclusions == &other.behavior_exclusions
            && service_audits == &other.service_audits
            && test_groups == &other.test_groups
    }

    pub fn program_roots(&self) -> Vec<symbols::SymbolHandle> {
        let mut roots: Vec<_> = self
            .root_bindings
            .iter()
            .map(|binding| binding.implementation_symbol)
            .collect();
        roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
        roots.dedup();
        roots
    }

    pub fn root_grant_selectors(&self) -> Vec<String> {
        self.grants
            .iter()
            .map(|grant| grant.selector.clone())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedBuildConfig {
    pub config: BuildConfig,
    pub root_binding_requests: Vec<language_semantics::declaration_selection::ExecutedRootBinding>,
    pub declared_product_selections:
        Vec<std::sync::Arc<language_semantics::declaration_selection::ProductDeclarationRequest>>,
    pub product_selection_requests:
        Vec<language_semantics::declaration_selection::ExecutedProductSelection>,
    pub native_formations: Vec<language_semantics::declaration_selection::NativeBindingFormation>,
    pub native_binding_requests:
        Vec<language_semantics::declaration_selection::ExecutedNativeBinding>,
    pub behavior_exclusion_requests:
        Vec<language_semantics::declaration_selection::ExecutedEnumExclusion>,
    pub project_declaration: Option<build_declarations::BuildDeclaration>,
    pub optimization_report_request: optimization_core::OptimizationReportRequest,
    pub evaluation_usage: Option<package_compilation::BuildEvaluationUsage>,
    pub observation_summary: Option<package_compilation::BuildObservationSummary>,
    pub selected_build_machine_symbol: Option<symbols::SymbolHandle>,
    pub generated_sources: Vec<build_output::PackageGeneratedSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildAdmission {
    pub application: Option<build_declarations::ApplicationDeclaration>,
    pub selected_build_machine_identity: Option<String>,
    pub restricted_build_requests:
        Vec<package_compilation::build_execution::RestrictedBuildRequest>,
}

impl BuildAdmission {
    #[must_use]
    pub fn for_target(mut self, profile: Option<target::TargetProfile>) -> Self {
        self.restricted_build_requests = self
            .restricted_build_requests
            .into_iter()
            .map(|request| request.for_target(profile))
            .collect();
        self
    }

    pub const fn artifact_only(&self) -> bool {
        match self.application.as_ref() {
            Some(application) => application.artifact_only,
            None => false,
        }
    }

    pub(crate) fn native_application_metadata(
        &self,
        settings: &EvaluatedBuildSettings,
        selected_target_profile: Option<target::TargetProfile>,
    ) -> target::NativeApplicationMetadata {
        target::NativeApplicationMetadata {
            name: self
                .application
                .as_ref()
                .map(|application| application.name.as_str().to_owned()),
            intent: settings.application_intent,
            identifier: settings.application_identifier.clone(),
            subsystem: settings.subsystem,
            elf_interpreter: selected_target_profile
                .and_then(|profile| settings.elf_interpreters.effective(profile)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndependentComponentSelection {
    pub selection: ProviderSelection,
    pub component: Option<semantic_vocabulary::PackageKeyIdentity>,
}

#[derive(Debug, Default)]
pub struct IndependentComponentDiscovery {
    pub selections: Vec<IndependentComponentSelection>,
    pub evaluation_usage: Option<package_compilation::BuildEvaluationUsage>,
}

impl ComputedBuildConfig {
    pub fn with_execution_observation(&self, execution: &Self) -> Result<Self, String> {
        if self.native_formations != execution.native_formations
            || self.native_binding_requests != execution.native_binding_requests
            || self
                .config
                .native_binding_evaluation
                .as_ref()
                .map(|evaluation| evaluation.terminal_code)
                != execution
                    .config
                    .native_binding_evaluation
                    .as_ref()
                    .map(|evaluation| evaluation.terminal_code)
        {
            return Err("source cohort execution changed native Build intent or its code".into());
        }
        let mut application = self.clone();
        application.config.native_binding_evaluation =
            execution.config.native_binding_evaluation.clone();
        if let Some(previous) = &self.config.native_binding_evaluation {
            let evidence = application
                .config
                .native_binding_evaluation
                .clone()
                .ok_or("source cohort native choices lost the current activation's evidence")?;
            for supply in &mut application.config.native_bindings {
                if std::sync::Arc::ptr_eq(&supply.evaluation, previous) {
                    supply.evaluation = evidence.clone();
                }
            }
        }
        application.evaluation_usage = execution.evaluation_usage;
        application.observation_summary = execution.observation_summary.clone();
        application.optimization_report_request = execution.optimization_report_request;
        Ok(application)
    }

    pub fn observation_identity(&self) -> Option<package_compilation::BuildObservationIdentity> {
        self.observation_summary
            .as_ref()
            .map(package_compilation::BuildObservationSummary::identity)
    }

    pub fn completed_outputs(
        &self,
    ) -> Result<Option<crate::RetainedBuildOutputs>, Vec<diagnostics::Diagnostic>> {
        self.observation_summary
            .as_ref()
            .map(crate::RetainedBuildOutputs::from_observation)
            .transpose()
            .map(Option::flatten)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
    }

    pub fn independent_components(&self) -> IndependentComponentDiscovery {
        IndependentComponentDiscovery {
            evaluation_usage: self.evaluation_usage,
            selections: self
                .config
                .provider_selections
                .iter()
                .filter(|selection| {
                    selection.composition_mode == provider_planning::CompositionMode::Independent
                })
                .map(|selection| IndependentComponentSelection {
                    selection: selection.clone(),
                    component: selection.provider_type.package,
                })
                .collect(),
        }
    }

    pub fn selected_machine_identity(
        &self,
        typed: &typed_trees::TypedTrees,
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
