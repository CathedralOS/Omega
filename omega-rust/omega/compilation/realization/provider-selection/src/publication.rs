//! Select canonical Build companions against the emitted Terminal demand catalog.
//!
//! Selection reads only canonical declarations, never typed trees or deferred
//! source work. Opaque producer correspondence travels unchanged to the caller;
//! publication discards it instead of interpreting source addresses.
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
pub struct PreparedProviderRealizations<Owner = ()> {
    pub provider_declaration: Option<String>,
    pub row_numeric_realizations: Vec<Option<String>>,
    pub owner: Owner,
}

#[derive(Debug, Clone)]
pub struct PreparedProviderCandidate<Owner = ()> {
    pub choice: ProviderCandidate<
        CanonicalProviderDeclaration,
        Result<PreparedProviderRealizations<Owner>, Vec<Diagnostic>>,
    >,
    pub schema_owner: String,
    pub explicitly_selected: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedTargetChoices<Owner = (), Origin = ()> {
    pub application_roots: Option<Vec<CanonicalProviderDeclaration>>,
    pub profile: target::TargetProfile,
    pub candidates: Vec<PreparedProviderCandidate<Owner>>,
    pub defaults: Vec<ProviderDeclaration<CanonicalProviderDeclaration, Origin>>,
    pub overrides: Vec<ProviderDeclaration<CanonicalProviderDeclaration, Origin>>,
    pub independent_components: Vec<std::sync::Arc<component_description::VerifiedComponent>>,
}

/// The whole canonical choice and its exact original candidate/declaration
/// correspondence. Consumers attach review metadata; they never select again.
pub struct SelectedTargetProviders<Owner, Origin> {
    pub selection: backend_plan::TerminalTargetProviderSelection,
    pub facts: effects::SelectedProviderPlanFacts,
    pub choices: Vec<crate::SelectedProvider<CanonicalProviderDeclaration, Owner, Origin>>,
}

#[derive(Debug, Clone)]
pub enum TargetProviderChoice {
    Prepared(PreparedTargetChoices),
    Selected {
        selected: std::sync::Arc<SelectedTargetProviderPublication>,
        application_roots: Option<Vec<CanonicalProviderDeclaration>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTargetProviderPublication {
    pub profile: target::TargetProfile,
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub application_roots: Option<Vec<terminal_psi::ProofDeclarationId>>,
    pub selection: backend_plan::TerminalTargetProviderSelection,
}

impl<Owner, Origin> PreparedTargetChoices<Owner, Origin> {
    /// Publication carries only canonical declarations. Original producer
    /// addresses are for the caller's review, not another selection input.
    pub fn without_correspondence(self) -> PreparedTargetChoices {
        let declarations =
            |rows: Vec<ProviderDeclaration<CanonicalProviderDeclaration, Origin>>| {
                rows.into_iter()
                    .map(|row| ProviderDeclaration {
                        target: row.target,
                        subject: row.subject,
                        provider_type: row.provider_type,
                        composition_mode: row.composition_mode,
                        origin: (),
                    })
                    .collect()
            };
        PreparedTargetChoices {
            application_roots: self.application_roots,
            profile: self.profile,
            candidates: self
                .candidates
                .into_iter()
                .map(|candidate| {
                    let ProviderCandidate {
                        plan,
                        declarations,
                        native_build,
                        owner,
                    } = candidate.choice;
                    PreparedProviderCandidate {
                        choice: ProviderCandidate {
                            plan,
                            declarations,
                            native_build,
                            owner: owner.map(|realizations| PreparedProviderRealizations {
                                provider_declaration: realizations.provider_declaration,
                                row_numeric_realizations: realizations.row_numeric_realizations,
                                owner: (),
                            }),
                        },
                        schema_owner: candidate.schema_owner,
                        explicitly_selected: candidate.explicitly_selected,
                    }
                })
                .collect(),
            defaults: declarations(self.defaults),
            overrides: declarations(self.overrides),
            independent_components: self.independent_components,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedTargetProviderPublication {
    deployed: bool,
    application_scopes: Vec<String>,
    targets: Vec<(String, Result<TargetProviderChoice, Vec<Diagnostic>>)>,
    assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
}

impl PreparedTargetProviderPublication {
    pub fn new(
        targets: Vec<(String, Result<TargetProviderChoice, Vec<Diagnostic>>)>,
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
                    .and_then(|choice| {
                        let prepared = match choice {
                            TargetProviderChoice::Prepared(prepared) => prepared,
                            TargetProviderChoice::Selected { selected, application_roots: roots } => {
                                if selected.terminal_psi != artifact.identity() {
                                    return Err(vec![Diagnostic::error(
                                        "retained provider choice belongs to another Terminal artifact",
                                    )]);
                                }
                                if selected.profile.target_name() != target {
                                    return Err(vec![Diagnostic::error(
                                        "retained provider choice belongs to another target",
                                    )]);
                                }
                                let expected_roots = roots.as_deref()
                                    .map(|roots| close_application_roots(roots, artifact))
                                    .transpose()
                                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                                if expected_roots != selected.application_roots
                                    || self.assumed_bindings != selected.selection.assumed_bindings
                                {
                                    return Err(vec![Diagnostic::error(
                                        "retained provider choice has different application roots or assumed bindings",
                                    )]);
                                }
                                application_roots = selected.application_roots.clone();
                                return Ok(selected.selection.clone());
                            }
                        };
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
                        ).map(|selected| selected.selection)
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

impl<Owner: Clone, Origin: Clone + Eq> PreparedTargetChoices<Owner, Origin> {
    /// Exact canonical entry roots for this target, independent of selection.
    pub fn close_application_roots(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Option<Vec<terminal_psi::ProofDeclarationId>>, Vec<Diagnostic>> {
        self.application_roots
            .as_deref()
            .map(|roots| close_application_roots(roots, artifact))
            .transpose()
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }

    /// Grow actual application demand through exact selected providers. Catalog
    /// retention alone never applies a requirement on an unrelated target.
    /// Each demanded schema still selects and admits its complete provider.
    pub fn select(
        self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        roots: Option<&[terminal_psi::ProofDeclarationId]>,
        assumed_bindings: &[effects::AssumedBindingContractSelection],
    ) -> Result<SelectedTargetProviders<Owner, Origin>, Vec<Diagnostic>> {
        let mut demanded = vec![false; self.candidates.len()];
        let mut previous = describe_selection::<Owner, Origin>(&[], artifact, assumed_bindings)?;
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
                let (facts, selected) = crate::admit(
                    selected,
                    self.independent_components.iter().map(AsRef::as_ref),
                )?;
                let selection = describe_selection(&selected, artifact, assumed_bindings)?;
                let choices = selected
                    .into_iter()
                    .map(|selected| {
                        let crate::ProviderCandidate {
                            plan,
                            declarations,
                            native_build,
                            owner,
                        } = selected.candidate;
                        let owner = owner?;
                        Ok(crate::SelectedProvider {
                            candidate: ProviderCandidate {
                                plan,
                                declarations,
                                native_build,
                                owner: owner.owner,
                            },
                            selected_by: selected.selected_by,
                        })
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                return Ok(SelectedTargetProviders {
                    selection,
                    facts,
                    choices,
                });
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

type SelectedCanonicalProvider<Owner, Origin> = crate::SelectedProvider<
    CanonicalProviderDeclaration,
    Result<PreparedProviderRealizations<Owner>, Vec<Diagnostic>>,
    Origin,
>;

fn describe_selection<Owner, Origin>(
    selected: &[SelectedCanonicalProvider<Owner, Origin>],
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
