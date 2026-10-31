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
    pub independent_components: Vec<std::sync::Arc<component_description::VerifiedComponent>>,
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
                        prepared.select(
                            artifact,
                            application_roots.as_deref(),
                            &self.assumed_bindings,
                        )
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
    /// Grow actual application demand through exact selected providers. Catalog
    /// retention alone never applies a requirement on an unrelated target.
    /// Each demanded schema still selects and admits its complete provider.
    pub fn select(
        self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        roots: Option<&[terminal_psi::ProofDeclarationId]>,
        assumed_bindings: &[effects::AssumedBindingContractSelection],
    ) -> Result<backend_plan::TerminalTargetProviderSelection, Vec<Diagnostic>> {
        let mut demanded = vec![false; self.candidates.len()];
        let mut previous = describe_selection(&[], artifact, assumed_bindings)?;
        loop {
            self.extend_application_demand(artifact, roots, &previous, &mut demanded)?;
            let candidates = self
                .candidates
                .iter()
                .zip(&demanded)
                .filter(|(_, demanded)| **demanded)
                .map(|(candidate, _)| candidate.choice.clone())
                .collect();
            let selected =
                crate::choose(candidates, self.profile, &self.defaults, &self.overrides)?;
            let proposed = describe_selection(&selected, artifact, assumed_bindings)?;
            if proposed == previous {
                // Intermediate choices discover calls, not evidence. Admit the
                // complete fixed point once, including all independent edges.
                let (_, selected) = crate::admit(
                    selected,
                    self.independent_components.iter().map(AsRef::as_ref),
                )?;
                return describe_selection(&selected, artifact, assumed_bindings);
            }
            previous = proposed;
        }
    }

    fn extend_application_demand(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        roots: Option<&[terminal_psi::ProofDeclarationId]>,
        selected: &backend_plan::TerminalTargetProviderSelection,
        demanded: &mut [bool],
    ) -> Result<(), Vec<Diagnostic>> {
        let module = artifact.module();
        let scope = &module.scalar_applications.proof_scope;
        let declaration = |callable| {
            scope.callable_declaration(callable).ok_or_else(|| {
                vec![Diagnostic::error(
                    "provider application demand lost its exact proof declaration",
                )]
            })
        };
        let mut roots = roots.map(<[_]>::to_vec);
        let mut edges = Vec::new();
        for row in &selected.checked_candidates {
            let requirement =
                declaration(terminal_psi::TargetMachineCallable::Boundary(row.boundary))?;
            let body = declaration(terminal_psi::TargetMachineCallable::Machine(row.candidate))?;
            edges.push((requirement, body));
            if let Some(roots) = &mut roots
                && !roots.contains(&body)
            {
                roots.push(body);
            }
        }
        // Assumptions retain their exact required/supplied requirement edge.
        // This discovers supply demand; final installation still owes the
        // original checked contracts, authority and complete alias admission.
        let plans = effects::SelectedProviderPlanFacts::from_selected_plans(
            selected.plans.iter().map(|row| row.plan.clone()).collect(),
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?
        .with_assumed_bindings(selected.assumed_bindings.clone());
        for (required, supplied) in plans
            .assumed_binding_coordinates(module)
            .map_err(|reason| vec![Diagnostic::error(reason)])?
        {
            edges.push((
                declaration(terminal_psi::TargetMachineCallable::Boundary(required))?,
                declaration(terminal_psi::TargetMachineCallable::Boundary(supplied))?,
            ));
        }
        let mut calls = scope
            .applications()
            .reachability(self.profile.target_name(), true, &edges, roots.as_deref())
            .calls;
        calls.sort_unstable();
        let mut invoked_owners = module
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.role == terminal_psi::BoundaryDeclarationRole::Callable)
            .filter(|boundary| {
                scope
                    .callable_declaration(terminal_psi::TargetMachineCallable::Boundary(
                        boundary.id,
                    ))
                    .is_some_and(|requirement| calls.binary_search(&requirement).is_ok())
            })
            .filter_map(|boundary| module.identities.text(boundary.owner.identity()))
            .collect::<Vec<_>>();
        invoked_owners.sort_unstable();
        invoked_owners.dedup();
        for (candidate, demanded) in self.candidates.iter().zip(demanded) {
            *demanded |= candidate.explicitly_selected
                || invoked_owners
                    .binary_search(&candidate.schema_owner.as_str())
                    .is_ok();
        }
        Ok(())
    }
}

type SelectedCanonicalProvider = crate::SelectedProvider<
    CanonicalProviderDeclaration,
    Result<PreparedProviderRealizations, Vec<Diagnostic>>,
>;

fn describe_selection(
    selected: &[SelectedCanonicalProvider],
    artifact: terminal_codec::TerminalArtifactView<'_>,
    assumed_bindings: &[effects::AssumedBindingContractSelection],
) -> Result<backend_plan::TerminalTargetProviderSelection, Vec<Diagnostic>> {
    let mut plans = Vec::with_capacity(selected.len());
    for selected in selected {
        let realizations = selected.candidate.owner.as_ref().map_err(Clone::clone)?;
        plans.push(backend_plan::TerminalTargetProviderPlan {
            plan: selected.candidate.plan.clone(),
            provider_declaration: realizations.provider_declaration.clone(),
            row_numeric_realizations: realizations.row_numeric_realizations.clone(),
        });
    }
    let mut selection = backend_plan::TerminalTargetProviderSelection::from_plans(plans, artifact)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    selection.assumed_bindings = assumed_bindings.to_vec();
    Ok(selection)
}

/// Rejoin exact prepared declarations to this immutable artifact, retaining
/// overloads and refusing both missing and competing executable coordinates.
fn close_application_roots(
    roots: &[CanonicalProviderDeclaration],
    artifact: terminal_codec::TerminalArtifactView<'_>,
) -> Result<Vec<terminal_psi::ProofDeclarationId>, String> {
    let module = artifact.module();
    let scope = &module.scalar_applications.proof_scope;
    let mut result = Vec::with_capacity(roots.len());
    for root in roots {
        let CanonicalProviderDeclaration::Machine { owner, callable } = root else {
            return Err("Build application root is not an exact ordinary machine".into());
        };
        // Declaration checking publishes application owners without requiring
        // their executable bodies. These canonical proof coordinates retain the
        // exact checked declaration; they grant no supplier or entry authority.
        let mut matches = scope
            .declarations
            .storage_slice()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, declaration)| {
                let terminal_psi::ProofScopeDeclaration::Callable(declaration) = declaration else {
                    return None;
                };
                (module.identities.text(declaration.machine.identity()) == Some(owner.as_str())
                    && module.identities.text(declaration.callable) == Some(callable.as_str()))
                .then_some(ordinal)
            });
        let ordinal = matches
            .next()
            .ok_or("Build application root has no retained checked proof declaration")?;
        if matches.next().is_some() {
            return Err("Build application root has competing checked proof declarations".into());
        }
        let coordinate = u32::try_from(ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(1))
            .ok_or("Build application proof coordinate namespace is exhausted")?;
        let root = terminal_psi::ProofDeclarationId(coordinate);
        if !result.contains(&root) {
            result.push(root);
        }
    }
    Ok(result)
}
