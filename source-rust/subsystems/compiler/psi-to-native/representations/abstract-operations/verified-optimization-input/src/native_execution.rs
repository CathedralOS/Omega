//! Immutable native execution facts and the proposal inputs for a selected scope.
use crate::native_proposal::TerminalNativeRealizationProposal;
use diagnostics::Diagnostic;
use std::collections::BTreeSet;

/// Provider rows selected for an execution scope. Stage 01 owns their issuance.
pub struct ScopedProviderBindings {
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub demands: BTreeSet<String>,
    pub compiler_builtins: Vec<provider_admission::TerminalCompilerBuiltinProposal>,
    pub boundary_application_coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
}

pub struct NativeExecutionFacts {
    checked: crate::ScopedProviderSelection,
    evidence: Option<std::sync::Arc<terminal_codec::EvidenceSeals>>,
    providers: ScopedProviderBindings,
}

impl TerminalNativeRealizationProposal {
    pub fn program_scope_request<'a>(
        &'a self,
        artifact: &'a terminal_codec::CanonicalTerminalArtifact,
        components: &[&crate::AdmittedNativeComponentCall<'_>],
    ) -> Result<crate::ProposalScopeRequest<'a>, Vec<Diagnostic>> {
        let adapters = self.providers().checked_adapters.as_slice();
        if components.iter().any(|component| {
            component.selection().receiver()
                != &terminal_codec::TerminalExecutionIdentity::portable(artifact.view().identity())
        }) {
            return Err(vec![Diagnostic::error(
                "component scope selection names another receiving artifact",
            )]);
        }
        for (position, component) in components.iter().enumerate() {
            if components[..position].iter().any(|earlier| {
                earlier.selection().receiving_boundary()
                    == component.selection().receiving_boundary()
            }) || adapters
                .iter()
                .filter(|adapter| component.replaces_local_adapter(adapter))
                .count()
                != 1
            {
                return Err(vec![Diagnostic::error(
                    "component scope selection has competing or missing exact local adapter coordinates",
                )]);
            }
        }
        let local = adapters
            .iter()
            .filter(|adapter| {
                !components
                    .iter()
                    .any(|component| component.replaces_local_adapter(adapter))
            })
            .cloned()
            .collect::<Vec<_>>();
        let entry = self.program_entry();
        let root = entry.physical_adapter().map_or(
            terminal_psi::TargetMachineCallable::Machine(entry.checked_entry().terminal_entry()),
            |physical| terminal_psi::TargetMachineCallable::Machine(physical.machine()),
        );
        let semantic_call = crate::entry_settlement::bind_selected_program_entry_call(
            artifact,
            entry,
            &self.providers().selection,
            &self.providers().intrinsic_executions,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        self.scope_request(artifact, root, semantic_call, local, None)
    }

    pub fn callable_scope_request<'a>(
        &'a self,
        artifact: &'a terminal_codec::CanonicalTerminalArtifact,
        entry: terminal_psi::TargetMachineCallable,
        previous: Option<&'a crate::ScopedProviderSelection>,
    ) -> Result<crate::ProposalScopeRequest<'a>, Vec<Diagnostic>> {
        let adapters = self.providers().checked_adapters.as_slice();
        let local = if let Some(previous) = previous {
            previous
                .authorization()
                .require_artifact(artifact)
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
            let scope = previous.scope();
            scope.require_proposal_context(artifact.view(), self)?;
            if scope.target_context().terminal_psi() != artifact.view().identity()
                || scope.target_context().target() != self.target_profile().target_name()
            {
                return Err(vec![Diagnostic::error(
                    "native callable preparation belongs to another artifact or target",
                )]);
            }
            scope.local_adapters()
        } else {
            adapters
        };
        self.scope_request(
            artifact,
            entry,
            None,
            local.to_vec(),
            previous.map(crate::ScopedProviderSelection::selection),
        )
    }

    fn scope_request<'a>(
        &'a self,
        artifact: &'a terminal_codec::CanonicalTerminalArtifact,
        root: terminal_psi::TargetMachineCallable,
        entry_call: Option<crate::SelectedProgramEntryCall>,
        local_adapters: Vec<provider_admission::SelectedProviderAdapter>,
        completed_selection: Option<&'a selected_provider_plans::SelectedProviderPlanFacts>,
    ) -> Result<crate::ProposalScopeRequest<'a>, Vec<Diagnostic>> {
        if self.program_entry().checked_entry().terminal_psi_identity()
            != artifact.view().identity()
        {
            return Err(vec![Diagnostic::error(
                "native proposal belongs to another canonical artifact",
            )]);
        }
        let targets = psi_target_layouts::terminal_target_context(
            artifact.view(),
            self.target_profile().target_name(),
        );
        let entry = targets
            .entry(artifact.view().identity(), artifact.semantic_module(), root)
            .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
        let assumptions = self
            .providers()
            .selection
            .assumed_binding_coordinates(artifact.semantic_module())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok(crate::ProposalScopeRequest {
            artifact: artifact.view(),
            entry,
            targets,
            entry_call,
            local_adapters,
            assumptions,
            proposal: self,
            completed_selection,
        })
    }

    pub fn settle_scoped_boundary_applications(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        scope: &crate::NativeExecutionScope,
        selection: &selected_provider_plans::SelectedProviderPlanFacts,
        provenance: &[provider_admission::CompilerIntrinsicPlanProvenance<'_>],
    ) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
        let coverage = crate::settlement::boundary_application_settlement::settle(
            artifact,
            selection,
            provenance,
            &self.boundary_applications().calling,
            &self.providers().native_bindings,
            Some(self.target_profile()),
            scope,
        )?;
        crate::settlement::comparison_settlement::settle_comparison_occurrences(
            artifact.semantic_module(),
            selection,
            &coverage,
            scope.machines(),
        )?;
        Ok(coverage)
    }
}

impl NativeExecutionFacts {
    pub fn from_scoped_providers(
        checked: crate::ScopedProviderSelection,
        providers: ScopedProviderBindings,
    ) -> Result<Self, Vec<Diagnostic>> {
        if providers.boundary_application_coverage.demands()
            != checked.scope().boundary_application_demands()
        {
            return Err(vec![Diagnostic::error(
                "native provider coverage belongs to another execution scope",
            )]);
        }
        Ok(Self {
            checked,
            evidence: None,
            providers,
        })
    }

    pub const fn checked_providers(&self) -> &crate::ScopedProviderSelection {
        &self.checked
    }

    /// The scalar applications this execution reads under `profile`'s admitted
    /// layouts. The caller completes exactly these before native lowering
    /// starts; Omega 00 refuses a completion that leaves one without a value.
    pub fn scalar_application_demand(
        &self,
        profile: target::TargetProfile,
    ) -> Result<Vec<semantic_vocabulary::ScalarApplicationId>, Vec<Diagnostic>> {
        let fail = |reason: String| diagnostics::error_in_context("native scalar demand", reason);
        profile.admit_address_representation().map_err(fail)?;
        let artifact = self.authorization().artifact();
        let targets =
            psi_target_layouts::terminal_target_context(artifact.view(), profile.target_name());
        let facts = profile
            .layout_target_facts()
            .ok_or_else(|| fail("selected native target has no representation facts".into()))?;
        let layouts = psi_target_layouts::admit_target_layouts(artifact.view(), &targets, &facts)
            .map_err(|error| fail(error.to_string()))?;
        self.scope()
            .scalar_application_demand(artifact.semantic_module(), &layouts)
            .map_err(fail)
    }

    pub const fn authorization(&self) -> &psi_authorization::AuthorizedTerminalArtifact {
        self.checked.authorization()
    }

    pub fn with_operation_evidence(
        mut self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        evidence: Option<std::sync::Arc<terminal_codec::EvidenceSeals>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.authorization()
            .require_artifact(artifact)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        if let Some(evidence) = &evidence {
            crate::native_proposal::validate_operation_evidence(artifact, evidence)?;
        }
        self.evidence = evidence;
        Ok(self)
    }

    pub const fn evidence(&self) -> Option<&std::sync::Arc<terminal_codec::EvidenceSeals>> {
        self.evidence.as_ref()
    }

    pub fn selection(&self) -> &selected_provider_plans::SelectedProviderPlanFacts {
        self.checked.selection()
    }
    pub fn source_selection(&self) -> &selected_provider_plans::SelectedProviderPlanFacts {
        self.scope().source_provider_selection()
    }
    pub const fn scope(&self) -> &crate::NativeExecutionScope {
        self.checked.scope()
    }
    pub fn external_bindings(&self) -> &[calling_conventions::ExternalBindingRow] {
        &self.providers.external_bindings
    }
    pub fn demands(&self) -> &BTreeSet<String> {
        &self.providers.demands
    }
    pub fn compiler_builtins(&self) -> &[provider_admission::TerminalCompilerBuiltinProposal] {
        &self.providers.compiler_builtins
    }
    pub fn local_adapters(&self) -> &[provider_admission::SelectedProviderAdapter] {
        self.scope().local_adapters()
    }
    pub fn boundary_application_coverage(
        &self,
    ) -> &boundary_applications::TerminalBoundaryApplicationCoverage {
        &self.providers.boundary_application_coverage
    }
}

/// Native compilation needs completed scalar values when its code reads one or
/// the program states any scalar condition, which lowering must find discharged.
pub fn requires_scalar_values(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    demand: &[semantic_vocabulary::ScalarApplicationId],
) -> bool {
    !demand.is_empty()
        || !artifact
            .semantic_module()
            .scalars
            .scalar_applications
            .conditions
            .is_empty()
}
