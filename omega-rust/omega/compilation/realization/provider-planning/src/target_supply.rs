//! Select the implementations and native bindings this program needs on one target.
//!
//! The module can contain implementations for several targets. Follow calls
//! from the program's entries so a Windows build does not require a Linux
//! implementation's system calls. At each boundary call, apply the declared
//! precedence: an explicit checked selection, then a native binding, then a
//! target default. A native binding ends that path. A checked selection adds
//! its implementation's calls to the walk.
//!
//! Missing or conflicting selections remain required but are not followed;
//! settlement rejects them. Preparation must never choose a body that
//! settlement would reject. Programs with no boundary calls need no supply,
//! and unsupported supply kinds refuse only programs that reach them.
//!
//! Settlement builds one plan per chosen provider. Each plan names the checked
//! implementation of every requirement in its slot's schema. See
//! `wiki/spec/build/provider_selection.md` for selection and dependency rules.

use build_time_evaluation::provider_declarations::{
    ProviderSelection, ProviderSelectionSubject, TargetProviderDefaults,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::TerminalModule;

/// One boundary requirement the accepted module declares: the callable the
/// module declares for it, the source declaration of that requirement and of
/// the boundary trait that owns it (the two declarations a provider
/// selection can name), and the authored `Trait::requirement` spelling for
/// messages. The coordinator fills the declared rows from
/// `AcceptedTerminalSource::boundary_requirements_declared`; the rows a
/// prepared supply holds are the ones the target's entries reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachedBoundaryRequirement {
    pub boundary: BoundaryMachineId,
    pub requirement: symbols::SymbolHandle,
    pub slot: symbols::SymbolHandle,
    pub name: String,
}

/// The machine the accepted module holds for one requirement of one chosen
/// provider (`AcceptedTerminalSource::supplied_requirements`). `provider`
/// and `requirement` are source declarations, compared by identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgedProvider {
    pub provider: symbols::SymbolHandle,
    pub requirement: symbols::SymbolHandle,
    pub machine: MachineId,
}

/// An explicit checked selection outranks a target-keyed native binding;
/// either outranks the target package's default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SupplyAuthority {
    TargetDefault,
    NativeBinding,
    Build,
}

/// One declared supply for a requirement on this target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSupply {
    pub authority: SupplyAuthority,
    pub selection: SupplySelection,
}

/// A checked implementation has a body to walk; an evaluated native binding
/// supplies exactly one requirement and ends that demand path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupplySelection {
    Checked(ProviderSelection),
    Native(crate::native_bindings::NativeBuildSupply),
}

impl ProviderSupply {
    pub fn checked(&self) -> Option<&ProviderSelection> {
        match &self.selection {
            SupplySelection::Checked(selection) => Some(selection),
            SupplySelection::Native(_) => None,
        }
    }

    fn description(&self) -> String {
        match &self.selection {
            SupplySelection::Checked(selection) => {
                format!("`{}`", selection.provider_type.authored_path)
            }
            SupplySelection::Native(supply) => format!(
                "native binding from `{}`",
                supply.selection.producer.declaration_path
            ),
        }
    }
}

/// One boundary requirement the target's entries reach and every supply
/// declared for it on this target, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandedRequirement {
    pub reached: ReachedBoundaryRequirement,
    pub supplies: Vec<ProviderSupply>,
    /// The machine the module holds for the one supply settlement would
    /// choose. `None` for a native leaf, when settlement would reject, and
    /// when the chosen provider has not been judged as a root of the
    /// module yet; the walk did not continue past this requirement then.
    pub realization: Option<MachineId>,
}

/// The demand of one accepted module on one target, or with no deployment
/// selected: a targetless check has a demand only when it reaches nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTargetSupply {
    target: Option<String>,
    requirements: Vec<DemandedRequirement>,
}

pub struct TargetSupplyRequest<'a> {
    /// The accepted module and the requirements it declares
    /// (`AcceptedTerminalSource::{module, boundary_requirements_declared}`).
    pub module: &'a TerminalModule,
    pub declared: &'a [ReachedBoundaryRequirement],
    /// The machines of the module the Product enters: the walk's roots.
    pub roots: &'a [MachineId],
    /// The machine the module holds for each requirement of each chosen
    /// provider it judged.
    pub judged: &'a [JudgedProvider],
    /// The evaluated Build's provider selections.
    pub build: &'a [ProviderSelection],
    /// Exact native requirements and the completed Build evaluations that
    /// produced their target-specific locators.
    pub native_bindings: &'a [crate::native_bindings::NativeBuildSupply],
    /// The defaults target packages retain.
    pub defaults: &'a [TargetProviderDefaults],
    /// The selected deployment, or no deployment for targetless checking.
    pub target: Option<&'a str>,
}

impl PreparedTargetSupply {
    pub fn prepare(request: TargetSupplyRequest<'_>) -> Result<Self, Vec<Diagnostic>> {
        let TargetSupplyRequest {
            module,
            declared,
            roots,
            judged,
            build,
            native_bindings,
            defaults,
            target,
        } = request;
        let scope = target.unwrap_or("<unscoped>");
        let mut refusals = unplanned_demands(module)
            .into_iter()
            .map(|demand| refuse(scope, demand))
            .collect::<Vec<_>>();
        // Sealed direct primitives retain their own boundary declarations,
        // but never become provider-selection slots. Their consumer joins the
        // explicit canonical requirement link when it realizes the operation.
        let provider_declarations = module
            .boundary_machines
            .iter()
            .filter(|boundary| !is_direct_compiler_primitive(boundary))
            .count();
        if provider_declarations != declared.len() {
            refusals.push(refuse(
                scope,
                "boundary declarations that differ from the requirements it was lowered from",
            ));
        }
        let defect = |reason: String| vec![refuse(scope, &format!("a call graph ({reason})"))];
        // Explicit Build selections commit their requirements on this target,
        // even in a provider-only package. Available defaults and unselected
        // declarations impose nothing. Continue through a selected body only
        // after the ordinary precedence/conflict decision below chooses it.
        let mut selected_declarations = build
            .iter()
            .filter(|selection| {
                selection
                    .target
                    .is_none_or(|selected| Some(selected.canonical_name()) == target)
            })
            .flat_map(|selection| selection_declarations(&selection.subject))
            .collect::<Vec<_>>();
        selected_declarations.sort_unstable();
        selected_declarations.dedup();
        let mut selected_requirements = declared
            .iter()
            .filter(|requirement| {
                selected_declarations
                    .binary_search(&SelectionDeclarationKey::slot(requirement.slot))
                    .is_ok()
                    || selected_declarations
                        .binary_search(&SelectionDeclarationKey::requirement(
                            requirement.requirement,
                        ))
                        .is_ok()
            })
            .map(|requirement| requirement.boundary)
            .collect::<Vec<_>>();
        selected_requirements.sort_unstable();
        selected_requirements.dedup();
        let mut roots = roots.to_vec();
        // Every boundary the walk has met, demanded or refused, so each is
        // decided once; `resolved` holds the ones the walk continues through.
        let mut met = Vec::<BoundaryMachineId>::new();
        let mut resolved = Vec::<(BoundaryMachineId, MachineId)>::new();
        let mut requirements = Vec::<DemandedRequirement>::new();
        loop {
            let graph = terminal_installation::SynchronousCallGraph::new(module, None)
                .and_then(|graph| graph.with_boundary_calls(resolved.iter().copied()))
                .map_err(defect)?;
            let scalar_roots = graph
                .scalar_evaluation_roots(&roots, target)
                .map_err(defect)?;
            let mut reached = graph
                .from_callables(
                    roots
                        .iter()
                        .copied()
                        .map(terminal_psi::TargetMachineCallable::Machine)
                        .chain(scalar_roots),
                )
                .map_err(defect)?
                .boundaries;
            reached.extend(selected_requirements.iter().copied());
            let before = resolved.len();
            for boundary in reached {
                if met.contains(&boundary) {
                    continue;
                }
                met.push(boundary);
                if let Some(declaration) = module.boundary_machine(boundary)
                    && is_direct_compiler_primitive(declaration)
                {
                    if declaration.numeric_intrinsic.is_some()
                        && let Some(target) = target
                        && let Err(diagnostics) =
                            crate::provider_planning::provider_rows::canonical_numeric_realization(
                                module,
                                declaration,
                                target,
                            )
                    {
                        refusals.extend(diagnostics);
                    }
                    continue;
                }
                let Some(requirement) = declared.iter().find(|row| row.boundary == boundary) else {
                    refusals.push(refuse(
                        scope,
                        "a boundary call with no requirement declaration",
                    ));
                    continue;
                };
                // No deployment is selected, so nothing says whose supply
                // this requirement would take.
                let Some(target) = target else {
                    refusals.push(Diagnostic::error(format!(
                        "boundary requirement `{}` requires an explicit deployment target",
                        requirement.name
                    )));
                    continue;
                };
                let mut supplies = build
                    .iter()
                    .map(|selection| (SupplyAuthority::Build, selection))
                    .chain(
                        defaults
                            .iter()
                            .filter(|row| row.target.as_deref().is_none_or(|row| row == target))
                            .flat_map(|row| &row.selections)
                            .map(|selection| (SupplyAuthority::TargetDefault, selection)),
                    )
                    .filter(|(_, selection)| {
                        selection
                            .target
                            .is_none_or(|selected| selected.canonical_name() == target)
                            && names_requirement(&selection.subject, requirement)
                    })
                    .map(|(authority, selection)| ProviderSupply {
                        authority,
                        selection: SupplySelection::Checked(selection.clone()),
                    })
                    .collect::<Vec<_>>();
                supplies.extend(
                    native_bindings
                        .iter()
                        .filter(|supply| {
                            let selected = &supply.selection;
                            selected.target.canonical_name() == target
                                && selected.subject == requirement.slot
                                && selected.requirement == requirement.requirement
                                && selected.application.is_none()
                        })
                        .map(|supply| ProviderSupply {
                            authority: SupplyAuthority::NativeBinding,
                            selection: SupplySelection::Native(supply.clone()),
                        }),
                );
                if let Err(demand) = unplanned_boundary(module, requirement) {
                    refusals.push(refuse(target, &format!("{demand} `{}`", requirement.name)));
                    continue;
                }
                let realization = chosen(&supplies).ok().and_then(|supply| {
                    let provider = supply.checked()?.provider_type.symbol;
                    judged
                        .iter()
                        .find(|row| {
                            row.provider == provider && row.requirement == requirement.requirement
                        })
                        .map(|row| row.machine)
                });
                if let Some(machine) = realization {
                    resolved.push((boundary, machine));
                    if selected_requirements.binary_search(&boundary).is_ok()
                        && !roots.contains(&machine)
                    {
                        roots.push(machine);
                    }
                }
                requirements.push(DemandedRequirement {
                    reached: requirement.clone(),
                    supplies,
                    realization,
                });
            }
            if resolved.len() == before {
                break;
            }
        }
        if !refusals.is_empty() {
            return Err(refusals);
        }
        // The module's declaration order, whatever order the walk met them in.
        requirements.sort_by_key(|requirement| requirement.reached.boundary);
        Ok(Self {
            target: target.map(str::to_owned),
            requirements,
        })
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    pub fn requirements(&self) -> &[DemandedRequirement] {
        &self.requirements
    }

    /// Whether the target's entries reach no boundary requirement, so this
    /// target has no provider to settle.
    pub fn demands_nothing(&self) -> bool {
        self.requirements.is_empty()
    }
}

/// Why a demanded requirement has no chosen supply.
enum Unchosen<'a> {
    Unsupplied,
    Competing(Vec<&'a ProviderSupply>),
}

/// The one supply at the highest authority that declared any.
fn chosen(supplies: &[ProviderSupply]) -> Result<&ProviderSupply, Unchosen<'_>> {
    let Some(authority) = supplies.iter().map(|supply| supply.authority).max() else {
        return Err(Unchosen::Unsupplied);
    };
    let ranked = supplies
        .iter()
        .filter(|supply| supply.authority == authority)
        .collect::<Vec<_>>();
    match ranked.as_slice() {
        [supply] => Ok(supply),
        _ => Err(Unchosen::Competing(ranked)),
    }
}

impl DemandedRequirement {
    /// The supply settlement would choose, when it would choose one.
    pub fn chosen(&self) -> Option<&ProviderSupply> {
        chosen(&self.supplies).ok()
    }
}

/// One demanded requirement and the single supply chosen for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledRequirement {
    pub reached: ReachedBoundaryRequirement,
    pub supply: ProviderSupply,
    /// The machine of the accepted module that realizes the requirement
    /// for the chosen provider, when the module holds it.
    pub realization: Option<MachineId>,
}

/// The supply chosen for every requirement one target's module demands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledTargetSupply {
    target: Option<String>,
    requirements: Vec<SettledRequirement>,
}

impl PreparedTargetSupply {
    /// Chooses the supply of each demanded requirement, or rejects the
    /// program naming every requirement that has none or more than one.
    pub fn settle(self) -> Result<SettledTargetSupply, Vec<Diagnostic>> {
        let Self {
            target,
            requirements,
        } = self;
        let mut rejections = Vec::new();
        let mut settled = Vec::with_capacity(requirements.len());
        for DemandedRequirement {
            reached,
            supplies,
            realization,
        } in requirements
        {
            let target = target.as_deref().ok_or_else(|| {
                vec![Diagnostic::error(
                    "provider settlement lost its deployment target",
                )]
            })?;
            match chosen(&supplies) {
                Ok(supply) => settled.push(SettledRequirement {
                    reached,
                    supply: supply.clone(),
                    realization,
                }),
                Err(Unchosen::Unsupplied) => rejections.push(Diagnostic::error(format!(
                    "boundary requirement `{}` has no provider on target `{target}`: the Build selects none and the target package declares no default",
                    reached.name
                ))),
                Err(Unchosen::Competing(ranked)) => {
                    let mut paths = ranked.iter().map(|supply| supply.description());
                    let first = paths.next().unwrap_or_default();
                    rejections.push(Diagnostic::error(format!(
                        "boundary requirement `{}` has conflicting providers on target `{target}`: {} declares {first} and {}",
                        reached.name,
                        match ranked[0].authority {
                            SupplyAuthority::Build => "the Build",
                            SupplyAuthority::NativeBinding => "native binding suppliers",
                            SupplyAuthority::TargetDefault => "the target package",
                        },
                        paths.collect::<Vec<_>>().join(" and "),
                    )));
                }
            }
        }
        if !rejections.is_empty() {
            return Err(rejections);
        }
        Ok(SettledTargetSupply {
            target,
            requirements: settled,
        })
    }
}

impl SettledTargetSupply {
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    pub fn requirements(&self) -> &[SettledRequirement] {
        &self.requirements
    }

    /// Normalize only selected native leaves, retaining the complete Build
    /// receipt and its identity table. A row grants no execution permission.
    pub fn native_bindings(
        &self,
        module: &TerminalModule,
    ) -> Result<NormalizedNativeSupplies, Vec<Diagnostic>> {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let mut rows = Vec::new();
        for requirement in &self.requirements {
            let SupplySelection::Native(supply) = &requirement.supply.selection else {
                continue;
            };
            let refuse = |reason| {
                vec![Diagnostic::error(format!(
                    "native supply for `{}`: {reason}",
                    requirement.reached.name
                ))]
            };
            let declaration = module
                .boundary_machine(requirement.reached.boundary)
                .and_then(|boundary| boundary.declaration.as_ref())
                .ok_or_else(|| {
                    refuse("the accepted module has no exact callable declaration".to_owned())
                })?;
            let callable = module
                .identities
                .text(declaration.source.callable)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    refuse("the accepted requirement has no complete callable identity".to_owned())
                })?;
            let binding =
                crate::native_bindings::normalize_native_binding(supply, callable, &mut identities)
                    .map_err(refuse)?;
            if Some(binding.target().target_name()) != self.target() {
                return Err(vec![Diagnostic::error(
                    "normalized native binding changed its settled deployment",
                )]);
            }
            rows.push(NormalizedNativeSupply {
                boundary: requirement.reached.boundary,
                binding,
            });
        }
        Ok(NormalizedNativeSupplies {
            identities: identities.build(),
            rows,
        })
    }

    /// The provider plans of the chosen supplies, one for each provider
    /// chosen for a slot: none for a module that demands nothing.
    ///
    /// provider_selection.md: "A selected plan must cover its complete schema
    /// and apply to the target", and plans "rejoin exact slot, realizing
    /// machine, provider value, schema, and requirement owners". So a plan
    /// has a checked adapter row for every requirement of the slot's schema,
    /// reached or not, each naming the machine the accepted module states as
    /// the chosen provider's candidate for that requirement. The provider is
    /// the declaration the module states for the machine settlement found for
    /// a reached requirement (`SettledRequirement::realization`), so the plan
    /// names the provider that was judged and nothing is looked up by name.
    /// A requirement of the schema with no such candidate, or with several,
    /// refuses the plan naming the requirement.
    pub fn plans(
        &self,
        planning: SupplyPlanning<'_>,
    ) -> Result<Vec<effects::provider_plan::ProviderPlan>, Vec<Diagnostic>> {
        if self.requirements.is_empty() {
            return Ok(Vec::new());
        }
        let target = self.target.as_deref().ok_or_else(|| {
            vec![Diagnostic::error(
                "provider settlement lost its deployment target",
            )]
        })?;
        // One plan for each provider chosen for a slot, however many of the
        // slot's requirements the target reaches.
        let mut chosen = Vec::<(symbols::SymbolHandle, Vec<&SettledRequirement>)>::new();
        let mut refusals = Vec::new();
        for requirement in &self.requirements {
            let Some(selection) = requirement.supply.checked() else {
                continue;
            };
            let ProviderSelectionSubject::BoundaryTrait(slot) = &selection.subject else {
                refusals.push(Diagnostic::error(format!(
                    "no product route yet: provider settlement for `{target}`: `{}` is supplied by `{}` through a selection of {}, and a provider plan is formed only for a selection of a whole boundary trait",
                    requirement.reached.name,
                    selection.provider_type.authored_path,
                    match selection.subject {
                        ProviderSelectionSubject::BoundaryRequirement(_) => "one requirement",
                        _ => "an operator family",
                    },
                )));
                continue;
            };
            match chosen.iter_mut().find(|(known, rows)| {
                *known == slot.symbol
                    && rows[0].supply.checked().is_some_and(|known| {
                        known.provider_type.symbol == selection.provider_type.symbol
                    })
            }) {
                Some((_, rows)) => rows.push(requirement),
                None => chosen.push((slot.symbol, vec![requirement])),
            }
        }
        let mut plans = Vec::with_capacity(chosen.len());
        for (slot, requirements) in chosen {
            match slot_plan(target, slot, &requirements, &planning) {
                Ok(plan) => plans.push(plan),
                Err(refusal) => refusals.push(refusal),
            }
        }
        if !refusals.is_empty() {
            return Err(refusals);
        }
        Ok(plans)
    }
}

/// Selected native leaves and the identities used by their retained receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedNativeSupplies {
    pub identities: terminal_psi::TerminalIdentityTable,
    pub rows: Vec<NormalizedNativeSupply>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedNativeSupply {
    pub boundary: BoundaryMachineId,
    pub binding: effects::provider_plan::EvaluatedNativeBinding,
}

/// What the plans of the chosen supplies are formed from.
#[derive(Clone, Copy)]
pub struct SupplyPlanning<'a> {
    /// The accepted module the supply was settled on: the provider
    /// candidates it states are what each plan's rows name.
    pub module: &'a TerminalModule,
    /// The schema of each slot a chosen provider supplies
    /// (`AcceptedTerminalSource::slot_schema`).
    pub schemas: &'a [SlotSchema],
    /// Canonical boundary requirements whose checked declarations defer to installation.
    pub installation_requirements: &'a [BoundaryMachineId],
}

impl SupplyPlanning<'_> {
    /// Use the selected body's own closure to resolve each authored installation bound.
    pub fn resolve_installation_reach(
        &self,
        selected: effects::SelectedProviderPlanFacts,
    ) -> Result<effects::SelectedProviderPlanFacts, Vec<Diagnostic>> {
        let module = self.module;
        let fail = |reason: String| vec![Diagnostic::error(reason)];
        if self.installation_requirements.is_empty() {
            return if module
                .root_service_reach
                .installation_dependencies
                .is_empty()
            {
                Ok(selected)
            } else {
                Err(fail(
                    "root installation reach has no authored requirement census".into(),
                ))
            };
        }
        let boundary = |id| {
            module
                .boundary_machine(id)
                .ok_or_else(|| fail(format!("installation reach lost exact boundary {id:?}")))
        };
        let service_name = |id| {
            module
                .services
                .binary_search_by_key(&id, |service| service.id)
                .ok()
                .and_then(|index| module.identities.text(module.services[index].identity))
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| fail(format!("installation reach lost exact service {id:?}")))
        };
        let graph = terminal_installation::SynchronousCallGraph::new(module, None).map_err(fail)?;
        let mut rows = Vec::new();
        for row in backend_plan::selected_checked_invocation_rows(selected.plans(), module)
            .map_err(fail)?
        {
            for candidate in row.candidates {
                let supplied = candidate.conformance;
                if self
                    .installation_requirements
                    .binary_search(&supplied.boundary)
                    .is_err()
                {
                    continue;
                }
                let required = boundary(supplied.boundary)?;
                let reach = graph
                    .service_reach_from(supplied.candidate, self.installation_requirements)
                    .map_err(fail)?;
                let dependencies = reach
                    .installation_dependencies
                    .iter()
                    .map(|dependency| {
                        module
                            .identities
                            .text(dependency.requirement_identity)
                            .filter(|identity| !identity.is_empty())
                            .map(str::to_owned)
                            .ok_or_else(|| {
                                fail(
                                    "installation reach dependency has no exact callable identity"
                                        .into(),
                                )
                            })
                    })
                    .collect::<Result<_, _>>()?;
                let requirement_identity = row
                    .plan
                    .identities
                    .text(row.row.requirement_identity)
                    .to_owned();
                if module.identities.text(required.identity) != Some(requirement_identity.as_str())
                {
                    return Err(fail(
                        "installation reach row differs from its selected requirement".into(),
                    ));
                }
                rows.push(provider_selection::InstallationReachRow {
                    requirement_identity,
                    provider_plan_report_identity: row.plan.report_fingerprint(),
                    provider_name: row.plan.provider_type.clone(),
                    realization_name: module
                        .identities
                        .text(supplied.candidate_identity)
                        .unwrap_or_default()
                        .to_owned(),
                    upper_bound: required
                        .published_service_ceiling
                        .iter()
                        .copied()
                        .map(service_name)
                        .collect::<Result<_, _>>()?,
                    concrete_reach: reach
                        .concrete
                        .into_iter()
                        .map(service_name)
                        .collect::<Result<_, _>>()?,
                    dependencies,
                });
            }
        }
        let (resolutions, identities) = provider_selection::resolve_installation_reach(rows)?;
        let selected = selected
            .with_installation_reach_resolutions(resolutions, &identities)
            .map_err(fail)?;
        selected.resolve_root_service_reach(module).map_err(fail)?;
        Ok(selected)
    }
}

/// The complete schema of one boundary trait a provider may be chosen for.
#[derive(Debug, Clone)]
pub struct SlotSchema {
    pub slot: symbols::SymbolHandle,
    pub surface: std::sync::Arc<effects::provider_plan::ServiceSurface>,
}

/// The plan of the one provider chosen for `slot`, which supplies each of
/// `requirements` (all reached on `target`, all with that provider).
fn slot_plan(
    target: &str,
    slot: symbols::SymbolHandle,
    requirements: &[&SettledRequirement],
    planning: &SupplyPlanning<'_>,
) -> Result<effects::provider_plan::ProviderPlan, Diagnostic> {
    use effects::provider_plan::{
        ProviderBinding, ProviderIdentityTable, ProviderPlan, ProviderPlanRow, ServiceSchema,
    };
    let module = planning.module;
    let text = |identity: terminal_psi::TerminalIdentityIndex| module.identities.text(identity);
    let first = requirements[0];
    let selection = first
        .supply
        .checked()
        .ok_or_else(|| Diagnostic::error("checked provider plan received a native binding"))?;
    let provider = &selection.provider_type;
    let refuse =
        |what: String| Diagnostic::error(format!("provider settlement for `{target}`: {what}"));
    let schema = planning
        .schemas
        .iter()
        .find(|schema| schema.slot == slot)
        .ok_or_else(|| {
            refuse(format!(
                "the slot `{}` names has no provider schema",
                first.reached.name
            ))
        })?;
    // The provider's declaration is the one the module states for the
    // machine settlement found for each reached requirement.
    let mut owner = None;
    for requirement in requirements {
        let machine = requirement.realization.ok_or_else(|| {
            refuse(format!(
                "`{}` is supplied by `{}`, which the accepted module does not hold as a judged provider",
                requirement.reached.name, provider.authored_path
            ))
        })?;
        let stated = module
            .provider_candidates
            .iter()
            .find(|candidate| {
                candidate.boundary == requirement.reached.boundary
                    && candidate.candidate == machine
                    && text(candidate.provider_identity) == Some(provider.canonical_path.as_str())
            })
            .ok_or_else(|| {
                refuse(format!(
                    "the accepted module states no candidate of `{}` for `{}`",
                    provider.authored_path, requirement.reached.name
                ))
            })?;
        match owner {
            None => owner = Some(stated.provider_declaration.clone()),
            Some(ref known) if *known == stated.provider_declaration => {}
            Some(_) => {
                return Err(refuse(format!(
                    "`{}` states candidates of `{}` under two declarations",
                    first.reached.name, provider.authored_path
                )));
            }
        }
    }
    let owner = owner.ok_or_else(|| refuse("a chosen provider supplies nothing".to_owned()))?;
    let surface = &schema.surface;
    let mut identities = ProviderIdentityTable::default();
    let mut rows = Vec::with_capacity(surface.methods.len());
    for method in &surface.methods {
        let requirement = surface.identity_text(method.requirement_identity);
        let mut candidates = module.provider_candidates.iter().filter(|candidate| {
            candidate.provider_declaration == owner
                && text(candidate.source_requirement_identity) == Some(requirement)
        });
        let named = format!("{}::{}", method.requirement_owner, method.name);
        let candidate = match (candidates.next(), candidates.next()) {
            (Some(candidate), None) => candidate,
            (None, _) => {
                return Err(refuse(format!(
                    "`{}` supplies `{named}` with no checked machine the accepted module states",
                    provider.authored_path
                )));
            }
            (Some(_), Some(_)) => {
                return Err(refuse(format!(
                    "`{}` supplies `{named}` with more than one checked machine",
                    provider.authored_path
                )));
            }
        };
        let machine = text(candidate.source_candidate_identity)
            .filter(|machine| !machine.is_empty())
            .ok_or_else(|| {
                refuse(format!(
                    "the machine `{}` supplies `{named}` with has no callable identity",
                    provider.authored_path
                ))
            })?;
        let requirement = identities.intern(requirement);
        rows.push(ProviderPlanRow {
            method: method.name.clone(),
            requirement_identity: requirement,
            declared_requirement_identity: requirement,
            calling_signature: None,
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: identities.intern(machine),
                machine_package_identity: candidate.source_candidate_package_identity,
            },
        });
    }
    let provider_declaration = text(owner.identity())
        .filter(|declaration| !declaration.is_empty())
        .ok_or_else(|| {
            refuse(format!(
                "`{}` has no declaration identity in the accepted module",
                provider.authored_path
            ))
        })?
        .to_owned();
    let plan = ProviderPlan {
        name: crate::satisfies_plan_name(
            target,
            surface.identity_text(surface.trait_name),
            &provider.canonical_path,
        ),
        provider_type: provider.canonical_path.clone(),
        provider_type_package_identity: provider.package,
        provider_declaration: Some(provider_declaration),
        row_numeric_realizations: vec![None; rows.len()],
        target: target.to_owned(),
        schema: ServiceSchema::from_declaration(std::sync::Arc::clone(surface)),
        identities,
        rows,
        origin_package_identity: provider.package,
        ..ProviderPlan::default()
    };
    let errors = plan.validate_against_schema();
    if !errors.is_empty() {
        return Err(refuse(errors.join("; ")));
    }
    Ok(plan)
}

fn refuse(target: &str, demand: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "no product route yet: target preparation for `{target}`: the accepted module demands {demand}, which has no provider or calling plan on this route"
    ))
}

/// A selection names a requirement through the boundary trait that owns it,
/// through the requirement itself, or through an operator family holding it.
/// Declarations are compared by identity, never by spelling.
///
/// A selection of one application of a trait, `Reading<17>`, names the
/// requirements of that application and of no other. A reached requirement
/// carries no application here yet (the accepted module declares none for a
/// trait with parameters), so such a selection names none of them rather
/// than every requirement of `Reading`.
fn names_requirement(
    subject: &ProviderSelectionSubject,
    requirement: &ReachedBoundaryRequirement,
) -> bool {
    let slot = SelectionDeclarationKey::slot(requirement.slot);
    let member = SelectionDeclarationKey::requirement(requirement.requirement);
    selection_declarations(subject).any(|declaration| declaration == slot || declaration == member)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SelectionDeclarationKey {
    Slot(u32, u32),
    Requirement(u32, u32),
}

impl SelectionDeclarationKey {
    fn slot(symbol: symbols::SymbolHandle) -> Self {
        Self::Slot(symbol.arena_index(), symbol.generation())
    }

    fn requirement(symbol: symbols::SymbolHandle) -> Self {
        Self::Requirement(symbol.arena_index(), symbol.generation())
    }
}

// Root preparation indexes only the selected family. Ordinary supply matching
// consumes these same identities and application guards without allocating.
fn selection_declarations(
    subject: &ProviderSelectionSubject,
) -> impl Iterator<Item = SelectionDeclarationKey> {
    let (single, family) = match subject {
        ProviderSelectionSubject::BoundaryTrait(identity) => (
            identity
                .application_identity
                .is_none()
                .then_some(SelectionDeclarationKey::slot(identity.symbol)),
            &[] as &[_],
        ),
        ProviderSelectionSubject::BoundaryRequirement(identity) => (
            identity
                .application_identity
                .is_none()
                .then_some(SelectionDeclarationKey::requirement(identity.symbol)),
            &[] as &[_],
        ),
        ProviderSelectionSubject::BoundaryOperatorFamily(family) => (None, family.coordinates()),
    };
    single.into_iter().chain(
        family
            .iter()
            .map(|coordinate| SelectionDeclarationKey::requirement(coordinate.symbol)),
    )
}

/// The module-level demand tables whose rows need a plan this route cannot
/// form yet. Each is named for what a program asked for, and an empty table
/// asks for nothing.
fn unplanned_demands(module: &TerminalModule) -> Vec<&'static str> {
    [
        (
            !module.external_providers.is_empty(),
            "an external provider realization",
        ),
        // A chosen provider's candidates are what its plan's rows name
        // (`SettledTargetSupply::plans`); an instance of a generic
        // requirement has no plan here yet.
        (
            !module.provider_instance_requirements.is_empty(),
            "a provider instance",
        ),
        (
            !module.boundary_application_candidates.is_empty()
                || !module.boundary_application_demands.is_empty(),
            "an applied boundary requirement",
        ),
        (
            !module.callback_applications.is_empty()
                || !module.installed_interrupt_registrations.is_empty(),
            "a callback",
        ),
        (
            !module.task_activation_demands.demands.is_empty(),
            "a task activation",
        ),
    ]
    .into_iter()
    .filter_map(|(demanded, name)| demanded.then_some(name))
    .collect()
}

/// A provider-executed requirement needs supply regardless of whether its
/// checked declaration returns normally or completes externally. The slot
/// schema retains that completion; choosing a provider grants no outcome
/// authority and creates no normal result.
fn unplanned_boundary(
    module: &TerminalModule,
    requirement: &ReachedBoundaryRequirement,
) -> Result<(), &'static str> {
    let declaration = module
        .boundary_machine(requirement.boundary)
        .ok_or("the undeclared boundary")?;
    if declaration.numeric_intrinsic.is_some() {
        return Err("the compiler numeric intrinsic");
    }
    if declaration.role != terminal_psi::BoundaryDeclarationRole::Callable {
        return Err("the contract-evidence boundary");
    }
    if declaration.declaration.as_ref().is_some_and(|callable| {
        callable.execution != terminal_psi::CheckedBoundaryExecution::Provider
    }) {
        return Err("the compiler-executed boundary");
    }
    Ok(())
}

/// This classification consumes an already checked declaration; it grants no
/// native execution. Target realization independently rejoins its core link.
fn is_direct_compiler_primitive(boundary: &terminal_psi::BoundaryMachineDeclaration) -> bool {
    boundary.declaration.as_ref().is_some_and(|declaration| {
        matches!(
            declaration.execution,
            terminal_psi::CheckedBoundaryExecution::HostedPrimitive { .. }
                | terminal_psi::CheckedBoundaryExecution::NumericPrimitive
        )
    })
}
