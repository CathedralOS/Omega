//! Native execution facts retained after provider selection.
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
    /// Target callable named by the checked program entry and its optional physical adapter.
    pub fn target_program_entry(&self) -> terminal_psi::TargetMachineCallable {
        let entry = self.program_entry();
        terminal_psi::TargetMachineCallable::Machine(entry.physical_adapter().map_or(
            entry.checked_entry().terminal_entry(),
            |adapter| adapter.machine(),
        ))
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
