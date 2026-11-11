/*
Canonical Build companion selection against the emitted Terminal demand catalog.

We read only canonical declarations, never typed trees or deferred source work.
Opaque producer correspondence travels unchanged to the caller and is discarded
at publication rather than interpreted as source addresses. Nominal owner and
callable overload remain separate from application identity, and receiving
artifact admission remains independent of this descriptive choice.
*/

/*
PreparedTargetChoices grows actual application demand to a fixed point through
exact selected providers:
- Product selection includes authored deployment roots and explicit choices,
  whereas an invocation demands only its executable call closure so unrelated
  Product bindings remain available choices rather than extra invocation roots.
- Parent choice relevance uses the same complete receiver application that the
  whole plan supplies, with the exact nominal join preceding export-key
  comparison.
- Selected operations that emit scalar comparisons rather than BoundaryCall
  instructions still retain their exact application demand in
  boundary_application_demands; we consume that canonical roster in the
  reachable body closure rather than inferring provider demand from instruction
  shape.
- Intermediate fixed-point choices discover calls rather than final evidence;
  once the fixed point stabilizes, we admit the complete selection once,
  including all independent component edges.
*/

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedProviderRealizations<Owner = ()> {
    pub provider_declaration: Option<String>,
    pub row_numeric_realizations: Vec<Option<String>>,
    pub owner: Owner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedProviderCandidate<Owner = ()> {
    pub choice: ProviderCandidate<
        CanonicalProviderDeclaration,
        Result<PreparedProviderRealizations<Owner>, Vec<Diagnostic>>,
    >,
    pub schema_owner: String,
    pub inherited_owners: Vec<String>, // inherited coverage does not itself demand a descendant's root slot
    pub explicitly_selected: bool,
}

impl<Owner> PreparedProviderCandidate<Owner> {
    fn accepts_choice<Origin>(
        &self,
        choice: &ProviderDeclaration<CanonicalProviderDeclaration, Origin>,
    ) -> bool {
        if choice
            .subject
            .selects_plan(&self.choice.declarations.schema, &self.choice.plan)
        {
            return true;
        }
        let crate::ProviderSubject::BoundaryTrait(identity) = &choice.subject else {
            return false;
        };
        let Ok(CanonicalProviderDeclaration::Trait(owner)) = &identity.declaration else {
            return false;
        };
        self.inherited_owners.contains(owner)
            && self
                .choice
                .plan
                .schema
                .declaration
                .receiver_applications
                .iter()
                .any(|receiver| {
                    let schema = &self.choice.plan.schema;
                    receiver.trait_package_identity == identity.package
                        && schema.identity_text(receiver.trait_name) == identity.canonical_path
                        && identity
                            .application_identity
                            .as_deref()
                            .is_none_or(|application| {
                                schema.identity_text(receiver.application) == application
                            })
                })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTargetChoices<Owner = (), Origin = ()> {
    pub application_roots: Option<Vec<CanonicalProviderDeclaration>>,
    pub profile: target::TargetProfile,
    pub candidates: Vec<PreparedProviderCandidate<Owner>>,
    pub defaults: Vec<ProviderDeclaration<CanonicalProviderDeclaration, Origin>>,
    pub overrides: Vec<ProviderDeclaration<CanonicalProviderDeclaration, Origin>>,
    pub independent_components: Vec<std::sync::Arc<component_description::VerifiedComponent>>,
}

#[derive(Clone, Copy)]
pub enum ProviderApplicationDemand<'roots> {
    Product(Option<&'roots [terminal_psi::ProofDeclarationId]>),
    Invocation(&'roots [terminal_psi::ProofDeclarationId]),
}

pub struct SelectedTargetProviders<Owner, Origin> {
    pub selection: backend_plan::TerminalTargetProviderSelection,
    pub facts: effects::SelectedProviderPlanFacts,
    pub demanded_plans: Vec<effects::provider_plan::ProviderPlan>, // fixed point's required candidate schemas, independent of which whole map wins
    pub invoked_declarations: Vec<terminal_psi::ProofDeclarationId>, // exact calls reached by this demand after the provider fixed point
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
    pub fn map_owner<Mapped>(
        self,
        mut map: impl FnMut(&effects::provider_plan::ProviderPlan, Owner) -> Mapped,
    ) -> PreparedTargetChoices<Mapped, Origin> {
        let candidates = self
            .candidates
            .into_iter()
            .map(|candidate| {
                let choice = candidate.choice;
                let owner = choice
                    .owner
                    .map(|realizations| PreparedProviderRealizations {
                        provider_declaration: realizations.provider_declaration,
                        row_numeric_realizations: realizations.row_numeric_realizations,
                        owner: map(&choice.plan, realizations.owner),
                    });
                PreparedProviderCandidate {
                    choice: ProviderCandidate {
                        plan: choice.plan,
                        declarations: choice.declarations,
                        native_build: choice.native_build,
                        canonical_float_map: choice.canonical_float_map,
                        owner,
                    },
                    schema_owner: candidate.schema_owner,
                    inherited_owners: candidate.inherited_owners,
                    explicitly_selected: candidate.explicitly_selected,
                }
            })
            .collect();
        PreparedTargetChoices {
            application_roots: self.application_roots,
            profile: self.profile,
            candidates,
            defaults: self.defaults,
            overrides: self.overrides,
            independent_components: self.independent_components,
        }
    }

    pub fn without_correspondence(self) -> PreparedTargetChoices {
        let choices = self.map_owner(|_, _| ());
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
            application_roots: choices.application_roots,
            profile: choices.profile,
            candidates: choices.candidates,
            defaults: declarations(choices.defaults),
            overrides: declarations(choices.overrides),
            independent_components: choices.independent_components,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedTargetProviderPublication {
    deployed: bool,
    targets: Vec<(String, Result<TargetProviderChoice, Vec<Diagnostic>>)>,
    assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
}

impl PreparedTargetProviderPublication {
    pub fn new(
        targets: Vec<(String, Result<TargetProviderChoice, Vec<Diagnostic>>)>,
        assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
        deployed: bool,
    ) -> Self {
        Self {
            deployed,
            targets,
            assumed_bindings,
        }
    }

    pub fn publish(
        self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        optimization_selection: optimization_core::OptimizationSelectionIdentity,
        realization_scopes: &[String],
    ) -> Vec<backend_plan::TerminalTargetProviderProposal> {
        self.targets
            .into_iter()
            .map(|(target, prepared)| {
                let mut application_roots = self.deployed.then(Vec::new);
                let application_required = realization_scopes.contains(&target);
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
                            ProviderApplicationDemand::Product(application_roots.as_deref()),
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

    pub fn select(
        self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        demand: ProviderApplicationDemand<'_>,
        assumed_bindings: &[effects::AssumedBindingContractSelection],
    ) -> Result<SelectedTargetProviders<Owner, Origin>, Vec<Diagnostic>> {
        let mut demanded = vec![false; self.candidates.len()];
        let mut previous = describe_selection::<Owner, Origin>(&[], artifact, assumed_bindings)?;
        loop {
            let invoked_declarations =
                self.extend_application_demand(artifact, demand, &previous, &mut demanded)?;
            let candidates: Vec<_> = self
                .candidates
                .iter()
                .zip(&demanded)
                .filter(|(_, demanded)| **demanded)
                .map(|(candidate, _)| candidate.choice.clone())
                .collect();
            let invocation_defaults;
            let invocation_overrides;
            let (defaults, overrides) = match demand {
                ProviderApplicationDemand::Product(_) => (&self.defaults[..], &self.overrides[..]),
                ProviderApplicationDemand::Invocation(_) => {
                    let relevant = |choice: &&ProviderDeclaration<
                        CanonicalProviderDeclaration,
                        Origin,
                    >| {
                        self.candidates
                            .iter()
                            .zip(&demanded)
                            .any(|(candidate, needed)| *needed && candidate.accepts_choice(choice))
                    };
                    invocation_defaults = self
                        .defaults
                        .iter()
                        .filter(relevant)
                        .cloned()
                        .collect::<Vec<_>>();
                    invocation_overrides = self
                        .overrides
                        .iter()
                        .filter(relevant)
                        .cloned()
                        .collect::<Vec<_>>();
                    (&invocation_defaults[..], &invocation_overrides[..])
                }
            };
            let selected = crate::choose(candidates, self.profile, defaults, overrides)?;
            let proposed = describe_selection(&selected, artifact, assumed_bindings)?;
            if proposed == previous {
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
                            canonical_float_map,
                            owner,
                        } = selected.candidate;
                        let owner = owner?;
                        Ok(crate::SelectedProvider {
                            candidate: ProviderCandidate {
                                plan,
                                declarations,
                                native_build,
                                canonical_float_map,
                                owner: owner.owner,
                            },
                            selected_by: selected.selected_by,
                        })
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                return Ok(SelectedTargetProviders {
                    selection,
                    facts,
                    demanded_plans: self
                        .candidates
                        .iter()
                        .zip(&demanded)
                        .filter(|(_, demanded)| **demanded)
                        .map(|(candidate, _)| candidate.choice.plan.clone())
                        .collect(),
                    choices,
                    invoked_declarations,
                });
            }
            previous = proposed;
        }
    }

    fn extend_application_demand(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        demand: ProviderApplicationDemand<'_>,
        selected: &backend_plan::TerminalTargetProviderSelection,
        demanded: &mut [bool],
    ) -> Result<Vec<terminal_psi::ProofDeclarationId>, Vec<Diagnostic>> {
        let module = artifact.module();
        let scope = &module.scalar_applications.proof_scope;
        let declaration = |callable| {
            scope.callable_declaration(callable).ok_or_else(|| {
                vec![Diagnostic::error(
                    "provider application demand lost its exact proof declaration",
                )]
            })
        };
        let mut roots = match demand {
            ProviderApplicationDemand::Product(roots) => roots.map(<[_]>::to_vec),
            ProviderApplicationDemand::Invocation(roots) => Some(roots.to_vec()),
        };
        let mut edges = Vec::new();
        for row in &selected.checked_candidates {
            let requirement =
                declaration(terminal_psi::TargetMachineCallable::Boundary(row.boundary))?;
            let body = declaration(terminal_psi::TargetMachineCallable::Machine(row.candidate))?;
            edges.push((requirement, body));
            if matches!(demand, ProviderApplicationDemand::Product(_))
                && let Some(roots) = &mut roots
                && !roots.contains(&body)
            {
                roots.push(body);
            }
        }
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
        let applications = scope.applications();
        let reachable = match demand {
            ProviderApplicationDemand::Product(_) => applications.reachability(
                self.profile.target_name(),
                true,
                &edges,
                roots.as_deref(),
            ),
            ProviderApplicationDemand::Invocation(_) => applications.invocation_reachability(
                self.profile.target_name(),
                &edges,
                roots.as_deref().unwrap_or_default(),
            ),
        };
        let mut calls = reachable.calls;
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
        let mut applications = module
            .boundary_application_demands
            .iter()
            .collect::<Vec<_>>();
        applications.sort_unstable_by_key(|demand| demand.terminal_operation());
        for machine in &module.machines {
            let Some(body_owner) = scope
                .callable_declaration(terminal_psi::TargetMachineCallable::Machine(machine.id))
            else {
                continue;
            };
            if !reachable.machines.contains(&body_owner) {
                continue;
            }
            for operation in machine.operations() {
                let Ok(index) = applications
                    .binary_search_by_key(&operation.id, |demand| demand.terminal_operation())
                else {
                    continue;
                };
                let requirement = applications[index].requirement();
                let owner = module
                    .identities
                    .text(requirement.declaration().identity())
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "provider application demand lost its exact requirement owner",
                        )]
                    })?;
                invoked_owners.push(owner);
            }
        }
        invoked_owners.sort_unstable();
        invoked_owners.dedup();
        for (candidate, demanded) in self.candidates.iter().zip(demanded) {
            let root_invoked = invoked_owners
                .binary_search(&candidate.schema_owner.as_str())
                .is_ok();
            let authored_map_demanded = match demand {
                ProviderApplicationDemand::Product(_) => candidate.explicitly_selected,
                ProviderApplicationDemand::Invocation(_) => {
                    candidate.explicitly_selected
                        && candidate
                            .inherited_owners
                            .iter()
                            .any(|owner| invoked_owners.binary_search(&owner.as_str()).is_ok())
                }
            };
            *demanded |= root_invoked || authored_map_demanded;
        }
        Ok(calls)
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
