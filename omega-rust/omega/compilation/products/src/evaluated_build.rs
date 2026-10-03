//! Checked build result: normalized settings and independently checked selections.

use build_time_evaluation::WireCompatibilityDemand;
use effects::AuthoredBehaviorExclusion;
use package_compilation::build_execution::build_value::EvaluatedBuildSettings;
use provider_planning::ProviderSelection;
use provider_planning::program_entry::RootBinding;
use representation_planning::OpaqueRepresentationSelection;
use trust_model::AuthoredComponentAssumptionAcceptance;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildConfig {
    pub settings: EvaluatedBuildSettings,
    /// CH10 ROOT GRANTS (GR3): the symbol paths the final build accepted
    /// via `b.accept_boundary<pkg::symbol>();` -- harvested STATICALLY
    /// from the build machine's marker calls (grants are declarations,
    /// not runtime effects; the evaluator serves the marker as a no-op).
    pub grants: Vec<trust_model::AuthoredRootGrant>,
    /// Executed provider-type choices for boundary slots, rejoined to the
    /// authoritative build machine. Provider settlement validates them against
    /// derived candidates before selection grants anything.
    pub provider_selections: Vec<ProviderSelection>,
    /// Exact named conformances activated as physical carriers for
    /// boundary-opaque values. The compiler derives shape from each carrier.
    pub opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    /// Channel/store compatibility demands. Each marker names the
    /// edge, format lineage, local and peer schemas, and the directional facts
    /// the final build requires.
    pub wire_compatibility_demands: Vec<WireCompatibilityDemand>,
    /// Target-owned inbound root slots bound by the authoritative build
    /// machine. The binding names an exact source machine; no entry discovery
    /// or naming convention participates once a binding is present.
    pub root_bindings: Vec<RootBinding>,
    /// Behavior exclusions (wiki/spec/build/behavior_exclusions.md): the
    /// authored `Build::exclude_crash(builder, ...)` product-admission requirements
    /// harvested from the root build machine's executed exclusion receipts,
    /// each retaining its exact toolchain case identity and authored span.
    /// These rows declare what the selected executable composition must not
    /// reach; the admission join itself consumes them later.
    pub behavior_exclusions: Vec<AuthoredBehaviorExclusion>,
    /// Accepted component-assumption digests
    /// (wiki/spec/build/component_publication.md): the authored
    /// `Build::accept_component_assumption(builder, ...)` declarations harvested
    /// statically from the root build machine's checked call scope. An
    /// attached independent-component description that binds an inseparable
    /// assumption verifies only when its exact digest appears here; the
    /// roster carries no accepted digest until the build authors one.
    pub accepted_component_assumptions: Vec<AuthoredComponentAssumptionAcceptance>,
}

impl BuildConfig {
    /// The selector paths of the accepted root grants, as trust reconstruction
    /// and provider checking name them.
    pub fn root_grant_selectors(&self) -> Vec<String> {
        self.grants
            .iter()
            .map(|grant| grant.selector.clone())
            .collect()
    }

    /// The accepted component-assumption digests. Provider settlement and any
    /// later replay verify attached component descriptions under this one set.
    pub fn accepted_component_assumption_digests(&self) -> std::collections::BTreeSet<[u8; 32]> {
        self.accepted_component_assumptions
            .iter()
            .map(|acceptance| acceptance.digest)
            .collect()
    }

    /// Project the effective checked-tree pruning selection into exact roots.
    /// A target product retains only its validated entry; a targetless product
    /// retains every authored root. An enabled selection with no roots remains
    /// `Some(empty)` so Psi rejects it rather than silently disabling pruning.
    /// This selects inputs only: Psi owns the transformation and its evidence.
    pub fn checked_tree_product_roots(
        &self,
        selected_target: Option<target::TargetProfile>,
        selected_entry: Option<&provider_planning::program_entry::SelectedCompilerProgramEntry>,
        rollback: &optimization_core::OptimizationRollback,
    ) -> Option<Vec<symbols::SymbolHandle>> {
        let effective = rollback.settle(&self.settings.optimizations);
        effective
            .effective()
            .contains(optimization_core::Optimization::CheckedTreeProductPruning)
            .then(|| {
                let roots: std::collections::HashSet<symbols::SymbolHandle> = match selected_target
                {
                    Some(_) => selected_entry
                        .iter()
                        .map(|entry| entry.source_signature().machine_symbol())
                        .collect(),
                    None => self
                        .root_bindings
                        .iter()
                        .map(|binding| binding.implementation_symbol)
                        .collect(),
                };
                roots.into_iter().collect()
            })
    }
}

/// Complete retained result of one admitted Build execution. Evaluation owns
/// producing it; product consumers do not depend on the evaluator's orchestration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedBuildConfig {
    pub config: BuildConfig,
    pub optimization_report_request: optimization_core::OptimizationReportRequest,
    pub evaluation_usage: Option<package_compilation::BuildEvaluationUsage>,
    pub observation_summary: Option<package_compilation::BuildObservationSummary>,
    pub selected_build_machine_symbol: Option<symbols::SymbolHandle>,
    pub generated_sources: Vec<build_output::PackageGeneratedSource>,
}

/// The admitted Build facts one compilation retains beside the shared evaluated
/// configuration. Each is derived for this continuation, so none belongs in the
/// configuration that target products and review clones share.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildAdmission {
    /// The validated authored `Build::application` declaration; its name
    /// supplies the `.app` basename and inner executable leaf at publication
    /// and its artifact-only intent narrows the admitted route.
    pub application: Option<build_declarations::ApplicationDeclaration>,
    /// Canonical semantic identity of the build machine actually evaluated,
    /// derived while the final typed declarations remain available. Custody
    /// must not rediscover it by short name.
    pub selected_build_machine_identity: Option<String>,
    /// The normalized restricted build-host requests the admitted activation
    /// asked of the host before it executed
    /// (wiki/spec/packages/acceptance.md#restricted-build-acceptance). This is
    /// admission intent in package vocabulary, never host paths or live grants.
    pub restricted_build_requests:
        Vec<package_compilation::build_execution::RestrictedBuildRequest>,
}

impl BuildAdmission {
    /// The Build evaluated once for every target; one target's compilation
    /// records the restricted requests as its own.
    #[must_use]
    pub fn for_target(mut self, profile: Option<target::TargetProfile>) -> Self {
        self.restricted_build_requests = self
            .restricted_build_requests
            .into_iter()
            .map(|request| request.for_target(profile))
            .collect();
        self
    }

    /// The validated `Build::artifact_only(builder)` intent. Package and workspace
    /// builds and missing build roots declare no application, so they answer
    /// `false`.
    pub const fn artifact_only(&self) -> bool {
        match self.application.as_ref() {
            Some(application) => application.artifact_only,
            None => false,
        }
    }

    /// Project the build-owned image metadata as one retention unit. The name
    /// comes from the application declaration; intent, signing identity and
    /// subsystem come from evaluated settings and remain distinct facts. The
    /// ELF loader is the realized target's effective selection: the build's
    /// explicit row, else the target environment's declared default.
    /// Retaining it here carries a defaulted path through the same proposal,
    /// report and package custody as an authored one.
    pub fn native_application_metadata(
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

/// An authored Independent selection and the provider's owning package.
/// No package means no dependency can answer it; self-deployment likewise
/// remains unanswerable. Neither case may silently become a Fused edge.
#[derive(Debug, Clone)]
pub struct IndependentComponentSelection {
    pub selection: ProviderSelection,
    pub component: Option<semantic_vocabulary::PackageKeyIdentity>,
}

/// Discovery retained before provider settlement, including when that later
/// stage rejects. Package review may publish the named component descriptions
/// and retry; its sponsor must still account for the first Build evaluation.
#[derive(Debug, Default)]
pub struct IndependentComponentDiscovery {
    pub selections: Vec<IndependentComponentSelection>,
    pub evaluation_usage: Option<package_compilation::BuildEvaluationUsage>,
}

impl ComputedBuildConfig {
    pub fn observation_identity(&self) -> Option<package_compilation::BuildObservationIdentity> {
        self.observation_summary
            .as_ref()
            .map(package_compilation::BuildObservationSummary::identity)
    }

    /// Select the completed-file product from this Build's observation.
    /// Compiler and package production share this projection; neither
    /// interprets the settlement roster or stages the build again.
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

    /// Preserve every authored Independent selection in evaluation order,
    /// including absent or self package identities for the consumer to reject.
    /// Discovery grants no component authority and does not settle providers.
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

    /// Require the Build's selected symbol to remain present and canonically
    /// named in the final typed continuation after generated-source extension.
    pub fn selected_machine_identity(
        &self,
        typed: &typed_trees::TypedTrees,
    ) -> Result<Option<String>, Vec<diagnostics::Diagnostic>> {
        self.selected_build_machine_symbol
            .map(|selected| {
                let machine = typed
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == selected)
                    .ok_or_else(|| {
                        vec![diagnostics::Diagnostic::error(
                            "selected build machine disappeared before final checking",
                        )]
                    })?;
                typed
                    .normalized_machine_overload_identity(machine)
                    .map(|identity| identity.identity().to_owned())
                    .ok_or_else(|| {
                        vec![diagnostics::Diagnostic::error(
                            "selected build machine has no canonical callable identity",
                        )]
                    })
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use optimization_core::{Optimization, OptimizationRollback, OptimizationSelections};

    #[test]
    fn checked_product_roots_follow_effective_selection_without_mutating_build() {
        let mut build = BuildConfig::default();
        let first = symbols::SymbolHandle::from_arena_index(1);
        let second = symbols::SymbolHandle::from_arena_index(2);
        build.root_bindings = [first, second, first]
            .into_iter()
            .map(|implementation_symbol| RootBinding {
                slot: "test::ProgramEntry".to_owned(),
                implementation: "same_debug_name".to_owned(),
                implementation_symbol,
            })
            .collect();
        assert_eq!(
            build.checked_tree_product_roots(None, None, &OptimizationRollback::default()),
            None
        );
        build.settings.optimizations =
            OptimizationSelections::new([Optimization::CheckedTreeProductPruning]).unwrap();
        let roots = build
            .checked_tree_product_roots(None, None, &OptimizationRollback::default())
            .unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&first));
        assert!(roots.contains(&second));
        let rollback =
            OptimizationRollback::new([Optimization::CheckedTreeProductPruning]).unwrap();
        assert_eq!(
            build.checked_tree_product_roots(None, None, &rollback),
            None
        );
        assert!(
            build
                .settings
                .optimizations
                .contains(Optimization::CheckedTreeProductPruning)
        );
    }

    #[test]
    fn selected_product_without_entry_keeps_empty_roots_for_stage_rejection() {
        let mut build = BuildConfig::default();
        build.settings.optimizations =
            OptimizationSelections::new([Optimization::CheckedTreeProductPruning]).unwrap();
        build.root_bindings.push(RootBinding {
            slot: "other_target::ProgramEntry".to_owned(),
            implementation: "Other::entry".to_owned(),
            implementation_symbol: symbols::SymbolHandle::from_arena_index(1),
        });
        let roots = build.checked_tree_product_roots(
            Some(target::TargetProfile::MacosArm64),
            None,
            &OptimizationRollback::default(),
        );
        assert_eq!(roots, Some(Vec::new()));
        let error = typed_trees_to_checked_trees::select_checked_tree_product(
            std::sync::Arc::new(checked_trees::CheckedTrees::default()),
            roots,
        )
        .unwrap_err();
        assert!(
            error[0]
                .message
                .contains("requires at least one bound product root")
        );
    }
}
