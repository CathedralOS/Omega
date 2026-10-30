//! Select canonical Build companions against the emitted Terminal demand catalog.
//!
//! These inputs contain no source handles, typed trees, or deferred source work.
//! Nominal owner and callable overload remain separate from application identity.
//! Receiving artifact admission remains independent of this descriptive choice.

use crate::{ProviderCandidate, ProviderDeclaration};
use diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalProviderDeclaration {
    Trait(String),
    Implementation(String),
    Data(String),
    Domain(String),
    Builtin(String),
    Machine { owner: String, callable: String },
    Operator { owner: String, callable: String },
}

#[derive(Debug, Clone)]
pub struct PreparedProviderRealizations {
    pub provider_declaration: Option<String>,
    pub row_numeric_realizations: Vec<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct PreparedProviderCandidate {
    pub choice: ProviderCandidate<
        CanonicalProviderDeclaration,
        Result<PreparedProviderRealizations, Vec<Diagnostic>>,
    >,
    pub schema_owner: String,
    pub explicitly_selected: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedTargetChoices {
    pub profile: target::TargetProfile,
    pub candidates: Vec<PreparedProviderCandidate>,
    pub defaults: Vec<ProviderDeclaration<CanonicalProviderDeclaration>>,
    pub overrides: Vec<ProviderDeclaration<CanonicalProviderDeclaration>>,
    pub independent_components: Vec<std::sync::Arc<component_description::VerifiedComponent>>,
}

#[derive(Debug, Clone)]
pub struct PreparedTargetProviderPublication {
    targets: Vec<(String, Result<PreparedTargetChoices, Vec<Diagnostic>>)>,
    assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
}

impl PreparedTargetProviderPublication {
    pub fn new(
        targets: Vec<(String, Result<PreparedTargetChoices, Vec<Diagnostic>>)>,
        assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
    ) -> Self {
        Self {
            targets,
            assumed_bindings,
        }
    }

    pub fn publish(
        self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        optimization_selection: optimization_core::OptimizationSelectionIdentity,
    ) -> Vec<backend_plan::TerminalTargetProviderProposal> {
        self.targets
            .into_iter()
            .map(|(target, prepared)| {
                let selection = prepared
                    .and_then(|prepared| {
                        if target::TargetProfile::from_canonical_target_name(&target).ok()
                            != Some(prepared.profile)
                        {
                            return Err(vec![Diagnostic::error(
                                "provider publication target differs from its authored inventory",
                            )]);
                        }
                        prepared.select(artifact)
                    })
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

impl PreparedTargetChoices {
    /// Demand filters complete candidates, never the declared family coordinate roster.
    pub fn select(
        self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<backend_plan::TerminalTargetProviderSelection, Vec<Diagnostic>> {
        let module = artifact.module();
        let demanded = self
            .candidates
            .into_iter()
            .filter(|candidate| {
                candidate.explicitly_selected
                    || module.boundary_machines.iter().any(|boundary| {
                        boundary.role == terminal_psi::BoundaryDeclarationRole::Callable
                            && module.identities.text(boundary.owner.identity())
                                == Some(candidate.schema_owner.as_str())
                    })
            })
            .map(|candidate| candidate.choice)
            .collect();
        let selected = crate::choose(demanded, self.profile, &self.defaults, &self.overrides)?;
        let (_, selected) = crate::admit(
            selected,
            self.independent_components.iter().map(AsRef::as_ref),
        )?;
        let mut plans = Vec::with_capacity(selected.len());
        for selected in selected {
            let realizations = selected.candidate.owner?;
            plans.push(backend_plan::TerminalTargetProviderPlan {
                plan: selected.candidate.plan,
                provider_declaration: realizations.provider_declaration,
                row_numeric_realizations: realizations.row_numeric_realizations,
            });
        }
        backend_plan::TerminalTargetProviderSelection::from_plans(plans, artifact)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }
}
