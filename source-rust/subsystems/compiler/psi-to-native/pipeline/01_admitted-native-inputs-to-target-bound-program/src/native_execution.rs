//! Issue the target-selected reach before the driver requests scalar values or lowers a body.
use diagnostics::Diagnostic;
use native_execution_scope::derive_native_scope;
use std::collections::BTreeSet;
use target_bound_program::{
    NativeExecutionScope, ScopedProviderSelection, SelectedProviderAdapter,
};
use target_bound_program::{ScopedProviderBindings, builtins::SelectedCompilerBuiltin};
use verified_optimization_input::TerminalNativeRealizationProposal;

mod provider_bindings;

/// A native program whose reachable provider bindings stage 01 completed.
pub struct PreparedNativeExecution {
    checked: ScopedProviderSelection,
    providers: ScopedProviderBindings,
}

impl PreparedNativeExecution {
    fn from_scoped_providers(
        checked: ScopedProviderSelection,
        providers: ScopedProviderBindings,
    ) -> Result<Self, Vec<Diagnostic>> {
        if providers.boundary_application_coverage.demands()
            != checked.scope().reach().boundary_application_demands()
        {
            return Err(vec![Diagnostic::error(
                "native provider coverage belongs to another execution scope",
            )]);
        }
        let artifact = checked.authorization().artifact();
        if let Some(evidence) = artifact.published_evidence() {
            // Validate even dormant seals before any lowering filters the module.
            if evidence.module != artifact.view().identity() {
                return Err(vec![Diagnostic::error(
                    "operation evidence belongs to another canonical artifact",
                )]);
            }
            terminal_codec::EvidenceSealValidator::new(artifact.semantic_module(), &evidence.seals)
                .validate_all()
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "canonical operation evidence: {error}"
                    ))]
                })?;
        }
        Ok(Self { checked, providers })
    }

    pub const fn checked_providers(&self) -> &ScopedProviderSelection {
        &self.checked
    }

    pub const fn authorization(&self) -> &psi_authorization::AdmittedTerminalArtifact {
        self.checked.authorization()
    }

    pub fn evidence(&self) -> Option<&std::sync::Arc<terminal_codec::EvidenceSeals>> {
        self.authorization().artifact().published_evidence()
    }

    pub fn selection(&self) -> &build_providers::selections::SelectedProviderPlanFacts {
        self.checked.selection()
    }

    pub const fn scope(&self) -> &NativeExecutionScope {
        self.checked.scope()
    }

    pub fn external_bindings(&self) -> &[calling_conventions::ExternalBindingRow] {
        &self.providers.external_bindings
    }

    pub fn demands(&self) -> &BTreeSet<String> {
        &self.providers.demands
    }

    pub fn compiler_builtins(&self) -> &[SelectedCompilerBuiltin] {
        &self.providers.compiler_builtins
    }

    pub fn local_adapters(&self) -> &[SelectedProviderAdapter] {
        self.scope().local_adapters()
    }

    pub fn boundary_application_coverage(
        &self,
    ) -> &boundary_applications::TerminalBoundaryApplicationCoverage {
        &self.providers.boundary_application_coverage
    }
}

pub fn prepare_program(
    proposal: &TerminalNativeRealizationProposal,
    authorization: &psi_authorization::AdmittedTerminalArtifact,
) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
    let binding = proposal.scope_source().program(authorization)?;
    let scope = derive_native_scope(binding)?;
    let program = provider_bindings::bind(proposal, authorization, scope)?;
    let root = proposal.scope_source().root();
    let progress_scope = match root {
        target_bound_program::NativeRoot::Program(_) => {
            psi_provider_selections::ProgressScope::Executable
        }
        target_bound_program::NativeRoot::Component { .. } => {
            psi_provider_selections::ProgressScope::OpenInterface
        }
    };
    let artifact = authorization.artifact();
    let mut candidates = artifact
        .semantic_module()
        .interfaces
        .provider_candidates
        .iter()
        .enumerate()
        .filter(|(index, _)| program.scope().reach().contains_candidate_position(*index))
        .map(|(_, row)| (row.boundary, row.candidate))
        .collect::<Vec<_>>();
    candidates.sort_unstable_by_key(|(slot, _)| *slot);
    if let Some(failure) = psi_provider_selections::invocation_progress_failure(
        artifact.view(),
        root.semantic_entry(),
        progress_scope,
        proposal
            .component_imports()
            .iter()
            .flat_map(|import| import.calls()),
        |slot| {
            candidates
                .binary_search_by_key(&slot, |(slot, _)| *slot)
                .ok()
                .map(|index| candidates[index].1)
        },
    ) {
        return Err(vec![Diagnostic::error(failure.to_string())]);
    }
    Ok(program)
}

/// An independently lowered callable inherits the selected providers of its owning execution.
pub struct BoundNativeCallable<'a> {
    proposal: &'a TerminalNativeRealizationProposal,
    execution: &'a PreparedNativeExecution,
    artifact: &'a terminal_codec::CanonicalTerminalArtifact,
    entry: terminal_psi::TargetMachineCallable,
    scope: NativeExecutionScope,
    coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
}

impl<'a> BoundNativeCallable<'a> {
    pub fn prepare(
        proposal: &'a TerminalNativeRealizationProposal,
        execution: &'a PreparedNativeExecution,
        artifact: &'a terminal_codec::CanonicalTerminalArtifact,
        entry: terminal_psi::TargetMachineCallable,
    ) -> Result<Self, Vec<Diagnostic>> {
        let binding =
            execution
                .checked_providers()
                .callable(artifact, proposal.scope_source(), entry)?;
        let scope = derive_native_scope(binding)?;
        let coverage = provider_bindings::callable_coverage(
            proposal,
            artifact,
            &scope,
            execution.selection(),
        )?;
        Ok(Self {
            proposal,
            execution,
            artifact,
            entry,
            scope,
            coverage,
        })
    }

    pub fn into_parts(
        self,
    ) -> (
        &'a TerminalNativeRealizationProposal,
        &'a PreparedNativeExecution,
        &'a terminal_codec::CanonicalTerminalArtifact,
        terminal_psi::TargetMachineCallable,
        NativeExecutionScope,
        boundary_applications::TerminalBoundaryApplicationCoverage,
    ) {
        (
            self.proposal,
            self.execution,
            self.artifact,
            self.entry,
            self.scope,
            self.coverage,
        )
    }
}
