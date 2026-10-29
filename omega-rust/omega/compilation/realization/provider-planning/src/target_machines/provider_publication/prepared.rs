//! Settle artifact demand against prepared declarations without reopening source.
//!
//! Candidate ownership keeps each exact plan, nominal owner and deferred failure
//! together. Invocation-local declaration coordinates are comparison inputs, not
//! a source lookup facility or a portable sidecar. Only closed proposals leave.

use diagnostics::Diagnostic;

pub(super) struct PreparedCandidate {
    pub(super) derived: crate::DerivedProviderPlan,
    pub(super) schema_owner: String,
    pub(super) closed_declarations:
        Result<backend_plan::TerminalTargetProviderPlan, Vec<Diagnostic>>,
    pub(super) explicitly_selected: bool,
}

pub(super) struct PreparedTarget {
    pub(super) profile: target::TargetProfile,
    pub(super) candidates: Vec<PreparedCandidate>,
    pub(super) defaults: Vec<crate::ProviderSelection>,
    pub(super) overrides: Vec<crate::ProviderSelection>,
    pub(super) selection_declarations: crate::provider_planning::PreparedSelectionDeclarations,
}

/// All authored targets retain their own preparation outcome. Sharing a Terminal
/// artifact does not authorize sharing different Build selection inputs.
pub struct PreparedTargetProviderPublication {
    targets: Vec<(String, Result<PreparedTarget, Vec<Diagnostic>>)>,
    assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
}

impl PreparedTargetProviderPublication {
    pub(super) fn new(
        targets: Vec<(String, Result<PreparedTarget, Vec<Diagnostic>>)>,
        assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
    ) -> Self {
        Self {
            targets,
            assumed_bindings,
        }
    }

    /// Demand selects whole providers; it never truncates plans to called rows.
    /// This join is descriptive and confers no native or host execution authority.
    pub fn publish(
        self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        optimization_selection: optimization_core::OptimizationSelectionIdentity,
    ) -> Vec<backend_plan::TerminalTargetProviderProposal> {
        self.targets
            .into_iter()
            .map(|(target, prepared)| {
                let selection = prepared
                    .and_then(|prepared| prepared.select(artifact))
                    .map(|mut selection| {
                        selection.assumed_bindings = self.assumed_bindings.clone();
                        selection
                    })
                    .map_err(|diagnostics| {
                        diagnostics
                            .into_iter()
                            .map(|diagnostic| diagnostic.message)
                            .collect()
                    });
                backend_plan::TerminalTargetProviderProposal {
                    target,
                    terminal_psi: artifact.identity(),
                    optimization_selection,
                    selection,
                }
            })
            .collect()
    }
}

impl PreparedTarget {
    fn select(
        self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<backend_plan::TerminalTargetProviderSelection, Vec<Diagnostic>> {
        let module = artifact.module();
        let demanded = self
            .candidates
            .iter()
            .filter(|candidate| {
                candidate.explicitly_selected
                    || module.boundary_machines.iter().any(|boundary| {
                        boundary.role == terminal_psi::BoundaryDeclarationRole::Callable
                            && module.identities.text(boundary.owner.identity())
                                == Some(candidate.schema_owner.as_str())
                    })
            })
            .map(|candidate| candidate.derived.clone())
            .collect::<Vec<_>>();
        let selected = crate::select_derived_provider_plans(
            &demanded,
            self.profile,
            &self.defaults,
            &self.overrides,
        )?;
        // Family cardinality, authored correspondence and independent-composition
        // admission still consume the complete actually selected roster.
        let (_, provenance) = self.selection_declarations.finish(selected, &[])?;
        let mut plans = Vec::with_capacity(provenance.len());
        for selected in provenance {
            let candidate = self
                .candidates
                .iter()
                .find(|candidate| {
                    candidate.derived.plan == selected.plan
                        && candidate.derived.provenance == selected.provider
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected provider lost its exact prepared declaration",
                    )]
                })?;
            plans.push(candidate.closed_declarations.clone()?);
        }
        backend_plan::TerminalTargetProviderSelection::from_plans(plans, artifact)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }
}
