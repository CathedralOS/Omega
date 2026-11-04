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
    /// Inherited coverage does not itself demand a descendant's root slot.
    pub inherited_owners: Vec<String>,
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
        // Parent choice relevance uses the same complete receiver application
        // the whole plan supplies. The exact nominal join precedes export-key
        // comparison; a display path cannot invent another receiver owner.
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

/// Product selection includes authored deployment roots and explicit choices.
/// A compiler invocation demands only its executable call closure: unrelated
/// Product bindings remain available choices, never extra invocation roots.
#[derive(Clone, Copy)]
pub enum ProviderApplicationDemand<'roots> {
    Product(Option<&'roots [terminal_psi::ProofDeclarationId]>),
    Invocation(&'roots [terminal_psi::ProofDeclarationId]),
}

/// The whole canonical choice and its exact original candidate/declaration
/// correspondence. Consumers attach review metadata; they never select again.
pub struct SelectedTargetProviders<Owner, Origin> {
    pub selection: backend_plan::TerminalTargetProviderSelection,
    pub facts: effects::SelectedProviderPlanFacts,
    /// The fixed point's required candidate schemas, independently of which
    /// whole map wins. Later review must not demand dormant source uses.
    pub demanded_plans: Vec<effects::provider_plan::ProviderPlan>,
    /// Exact calls reached by this demand after the provider fixed point.
    /// These declaration coordinates describe demand, never invocation authority.
    pub invoked_declarations: Vec<terminal_psi::ProofDeclarationId>,
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
    /// Save additional checks with each possible implementation. Selection
    /// carries them back to the caller only for implementations it chooses.
    /// Keep existing declaration errors in place for the same later decision.
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

    /// Publication carries only canonical declarations. Original producer
    /// addresses are for the caller's review, not another selection input.
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
                // Declaring a Build row retains a choice, not a request to
                // realize its backend. The operation owns the actual cohort.
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
        demand: ProviderApplicationDemand<'_>,
        assumed_bindings: &[effects::AssumedBindingContractSelection],
    ) -> Result<SelectedTargetProviders<Owner, Origin>, Vec<Diagnostic>> {
        let debug = std::env::var_os("OMEGA_PROVIDER_DEBUG").is_some();
        if debug {
            for (kind, choices) in [
                ("default", &self.defaults[..]),
                ("override", &self.overrides[..]),
            ] {
                for choice in choices {
                    eprintln!(
                        "[prov] target-choice kind={kind} target={:?} subject={:?} provider={:?} composition={:?}",
                        choice.target,
                        choice.subject,
                        choice.provider_type,
                        choice.composition_mode
                    );
                }
            }
        }
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
                    // Keep invalid choices for every demanded schema too. A
                    // missing requested provider is a refusal, not permission
                    // to fall back to another available implementation.
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
            let selected = match crate::choose(candidates, self.profile, defaults, overrides) {
                Ok(selected) => selected,
                Err(diagnostics) => {
                    if debug {
                        eprintln!(
                            "[prov] target-choice-refused target={} diagnostics={diagnostics:?}",
                            self.profile.target_name()
                        );
                    }
                    return Err(diagnostics);
                }
            };
            if debug {
                for choice in &selected {
                    let selected_by = match &choice.selected_by {
                        crate::SelectionProvenance::BuildOverride(_) => "build-override",
                        crate::SelectionProvenance::TargetDefault(_) => "target-default",
                        crate::SelectionProvenance::UniqueCoveringCandidate => {
                            "unique-covering-candidate"
                        }
                        crate::SelectionProvenance::NativeBuild => "native-build",
                    };
                    eprintln!(
                        "[prov] target-chosen target={} plan={} provider={} selected_by={selected_by}",
                        self.profile.target_name(),
                        choice.candidate.plan.name,
                        choice.candidate.plan.provider_type
                    );
                }
            }
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
        let debug = std::env::var_os("OMEGA_PROVIDER_DEBUG").is_some();
        if debug {
            eprintln!(
                "[prov] application-demand-roots target={} roots={roots:?} reachable_machines={:?} reachable_calls={:?}",
                self.profile.target_name(),
                reachable.machines,
                reachable.calls
            );
            for application in &module.boundary_application_demands {
                let operation = application.terminal_operation();
                let machine = module.machines.iter().find(|machine| {
                    machine
                        .operations()
                        .any(|candidate| candidate.id == operation)
                });
                let declaration = machine.and_then(|machine| {
                    scope.callable_declaration(terminal_psi::TargetMachineCallable::Machine(
                        machine.id,
                    ))
                });
                let reached = declaration.is_some_and(|owner| reachable.machines.contains(&owner));
                let requirement = application.requirement();
                eprintln!(
                    "[prov] application-demand target={} operation={operation:?} owner_machine={:?} owner_declaration={declaration:?} reachable={reached} requirement_owner={:?} requirement={requirement:?}",
                    self.profile.target_name(),
                    machine.map(|machine| machine.id),
                    module.identities.text(requirement.declaration().identity())
                );
            }
        }
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
        // A selected operation need not remain a BoundaryCall instruction:
        // comparisons retain their exact application beside the emitted scalar
        // operation. Consume that canonical roster in the same reachable body
        // closure, rather than inferring provider demand from instruction shape.
        // Merely retaining an application in a dormant target body grants no
        // demand on this target. Provider bodies join this walk at the next
        // fixed-point iteration, just as their ordinary calls do.
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
            // Merely declaring an unused descendant cannot create another
            // demanded slot. Only an authored whole map may supply an invoked
            // ancestor without an invocation of its own root requirement.
            if debug {
                eprintln!(
                    "[prov] candidate-demand target={} plan={} provider={} schema_owner={} root_invoked={} explicitly_selected={} authored_map_demanded={} previously_demanded={} default_matches={} override_matches={}",
                    self.profile.target_name(),
                    candidate.choice.plan.name,
                    candidate.choice.plan.provider_type,
                    candidate.schema_owner,
                    root_invoked,
                    candidate.explicitly_selected,
                    authored_map_demanded,
                    *demanded,
                    self.defaults
                        .iter()
                        .filter(|choice| candidate.accepts_choice(choice))
                        .count(),
                    self.overrides
                        .iter()
                        .filter(|choice| candidate.accepts_choice(choice))
                        .count()
                );
            }
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
