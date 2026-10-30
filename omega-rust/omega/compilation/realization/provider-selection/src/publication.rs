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
    pub application_roots: Option<Vec<CanonicalProviderDeclaration>>,
    pub profile: target::TargetProfile,
    pub candidates: Vec<PreparedProviderCandidate>,
    pub defaults: Vec<ProviderDeclaration<CanonicalProviderDeclaration>>,
    pub overrides: Vec<ProviderDeclaration<CanonicalProviderDeclaration>>,
}

#[derive(Debug, Clone)]
pub struct PreparedTargetProviderPublication {
    deployed: bool,
    application_scopes: Vec<String>,
    targets: Vec<(String, Result<PreparedTargetChoices, Vec<Diagnostic>>)>,
    assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
}

impl PreparedTargetProviderPublication {
    pub fn new(
        targets: Vec<(String, Result<PreparedTargetChoices, Vec<Diagnostic>>)>,
        assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
        deployed: bool,
        application_scopes: Vec<String>,
    ) -> Self {
        Self {
            deployed,
            application_scopes,
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
                let mut application_roots = self.deployed.then(Vec::new);
                let application_required = self.application_scopes.contains(&target);
                let selection = prepared
                    .and_then(|prepared| {
                        if target::TargetProfile::from_canonical_target_name(&target).ok()
                            != Some(prepared.profile)
                        {
                            return Err(vec![Diagnostic::error(
                                "provider publication target differs from its authored inventory",
                            )]);
                        }
                        if let Some(roots) = &prepared.application_roots {
                            application_roots = Some(
                                close_application_roots(roots, artifact)
                                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
                            );
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
                    application_roots,
                    application_required,
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
        let (_, selected) = crate::admit(selected, &[])?;
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

/// Rejoin exact prepared declarations to this immutable artifact, retaining
/// overloads and refusing both missing and competing executable coordinates.
fn close_application_roots(
    roots: &[CanonicalProviderDeclaration],
    artifact: terminal_codec::TerminalArtifactView<'_>,
) -> Result<Vec<terminal_psi::TargetMachineCallable>, String> {
    let module = artifact.module();
    let mut result = Vec::with_capacity(roots.len());
    for root in roots {
        let CanonicalProviderDeclaration::Machine { owner, callable } = root else {
            return Err("Build application root is not an exact ordinary machine".into());
        };
        let declarations = module
            .machines
            .iter()
            .filter_map(|machine| {
                machine.declaration.as_ref().map(|declaration| {
                    (
                        terminal_psi::TargetMachineCallable::Machine(machine.id),
                        declaration,
                    )
                })
            })
            .chain(module.boundary_machines.iter().filter_map(|boundary| {
                boundary.declaration.as_ref().map(|declaration| {
                    (
                        terminal_psi::TargetMachineCallable::Boundary(boundary.id),
                        &declaration.source,
                    )
                })
            }));
        let mut matches = declarations
            .filter(|(_, declaration)| {
                module.identities.text(declaration.machine.identity()) == Some(owner.as_str())
                    && module.identities.text(declaration.callable) == Some(callable.as_str())
            })
            .map(|(callable, _)| callable);
        let root = matches
            .next()
            .ok_or("Build application root has no emitted checked declaration")?;
        if matches.next().is_some() {
            return Err("Build application root has competing emitted declarations".into());
        }
        if !result.contains(&root) {
            result.push(root);
        }
    }
    Ok(result)
}
