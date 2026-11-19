// Additional operations for the retired V1 compilation coordinator.
pub use build_evaluation_result::{
    BuildConfig, BuildConstants, BuildSubjectValidation, ComputedBuildConfig, EvaluatedBuild,
};
pub use source_declarations::source_selections::ServiceAuditRegistration;
use package_compilation::build_execution::build_value::EvaluatedBuildSettings;
use source_compiler::provider_choice_inputs::ProviderSelection;

pub struct ProviderDeployment {
    pub roots: Option<Vec<(String, symbols::SymbolHandle)>>, // None = reusable Source checking; Some(empty) = provider-only deployment
}

pub trait BuildConfigExt: Sized {
    fn same_source_choices(&self, other: &Self) -> bool;
    fn program_roots(&self) -> Vec<symbols::SymbolHandle>;
}

impl BuildConfigExt for BuildConfig {
    fn same_source_choices(&self, other: &Self) -> bool {
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

    fn program_roots(&self) -> Vec<symbols::SymbolHandle> {
        let mut roots: Vec<_> = self
            .root_bindings
            .iter()
            .map(|binding| binding.implementation_symbol)
            .collect();
        roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
        roots.dedup();
        roots
    }
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

pub trait ComputedBuildConfigExt: Sized {
    fn with_execution_observation(&self, execution: &Self) -> Result<Self, String>;
    fn observation_identity(&self) -> Option<package_compilation::BuildObservationIdentity>;
    fn completed_outputs(
        &self,
    ) -> Result<Option<crate::RetainedBuildOutputs>, Vec<diagnostics::Diagnostic>>;
    fn independent_components(&self) -> IndependentComponentDiscovery;
}

impl ComputedBuildConfigExt for ComputedBuildConfig {
    fn with_execution_observation(&self, execution: &Self) -> Result<Self, String> {
        if self.accepted_build.is_some() && execution.accepted_build.is_none() {
            return Err("source cohort execution lost its accepted Build judgment".into());
        }
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
        application.accepted_build = execution.accepted_build.clone();
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

    fn observation_identity(&self) -> Option<package_compilation::BuildObservationIdentity> {
        self.observation_summary
            .as_ref()
            .map(package_compilation::BuildObservationSummary::identity)
    }

    fn completed_outputs(
        &self,
    ) -> Result<Option<crate::RetainedBuildOutputs>, Vec<diagnostics::Diagnostic>> {
        self.observation_summary
            .as_ref()
            .map(crate::RetainedBuildOutputs::from_observation)
            .transpose()
            .map(Option::flatten)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
    }

    fn independent_components(&self) -> IndependentComponentDiscovery {
        IndependentComponentDiscovery {
            evaluation_usage: self.evaluation_usage,
            selections: self
                .config
                .provider_selections
                .iter()
                .filter(|selection| {
                    selection.composition_mode
                        == declaration_contracts::selection::CompositionMode::Independent
                })
                .map(|selection| IndependentComponentSelection {
                    selection: selection.clone(),
                    component: selection.provider_type.package,
                })
                .collect(),
        }
    }
}
