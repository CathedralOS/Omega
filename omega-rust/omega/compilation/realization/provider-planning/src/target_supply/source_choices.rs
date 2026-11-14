//! What one target must supply for a Product, decided by what the Product's
//! accepted Terminal module demands on that target.
//!
//! A program needs a provider for a boundary requirement on a target only
//! when its entry reaches that requirement there. The accepted module is one
//! target-neutral module: beside the entry's closure it holds the providers
//! chosen for every target, so the requirements it declares are more than
//! any one target demands. A Linux provider's raw system-call requirement is
//! declared in the module a Windows product is made from, and Windows must
//! not be asked to supply it.
//!
//! So the demand of a target is a walk of the module's call graph
//! (`terminal_installation::SynchronousCallGraph`). It starts at the
//! Product's entries. Each boundary requirement it meets is demanded and is
//! paired with the supply declared for the target: the provider selections
//! the evaluated Build made and the defaults the target package retains.
//! Both are the same `ProviderSelection`; only the authority that declared
//! them differs. Where exactly one supply stands at the highest authority,
//! the walk continues into the machine the module holds for that provider
//! and that requirement, because whatever the chosen provider calls is
//! demanded too (provider_selection.md: "A selected provider's exact boundary
//! calls create static transitive provider dependencies. Provider planning
//! closes those rows under target selection"). A requirement that has no
//! such supply, or whose chosen provider the module does not hold yet, ends
//! the walk there and is still reported as demanded.
//!
//! A module whose entries reach nothing prepares an empty supply, and that
//! is a finding about the artifact rather than a skipped stage. A module that
//! demands a kind of supply nothing here can plan yet is refused by the name
//! of what it demands, so the refusal reaches only programs that need it.
//!
//! Preparation follows a choice only where settlement would make it.
//! Settlement rejects: each demanded requirement must have exactly one
//! supply at the highest authority that declared any, and a requirement with
//! none or with several rejects the program
//! (wiki/spec/build/provider_selection.md, "Missing, duplicate, incompatible,
//! or unadmitted selected supply rejects rather than choosing a body by
//! spelling or source order"). Compatibility source choices must rejoin the
//! already published target choice before canonical provider realization.

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

/// Who declared a supply. An explicit Build selection outranks the target
/// package's default for the same slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SupplyAuthority {
    TargetDefault,
    Build,
}

/// One declared provider for a slot on this target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSupply {
    pub authority: SupplyAuthority,
    pub selection: ProviderSelection,
}

/// One boundary requirement the target's entries reach and every supply
/// declared for it on this target, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandedRequirement {
    pub reached: ReachedBoundaryRequirement,
    pub supplies: Vec<ProviderSupply>,
    /// The machine the module holds for the one supply settlement would
    /// choose. `None` when settlement would reject the requirement, and
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
            defaults,
            target,
        } = request;
        let scope = target.unwrap_or("<unscoped>");
        let mut refusals = unplanned_demands(module)
            .into_iter()
            .map(|demand| refuse(scope, demand))
            .collect::<Vec<_>>();
        if module.boundary_machines.len() != declared.len() {
            refusals.push(refuse(
                scope,
                "boundary declarations that differ from the requirements it was lowered from",
            ));
        }
        let defect = |reason: String| vec![refuse(scope, &format!("a call graph ({reason})"))];
        // Every boundary the walk has met, demanded or refused, so each is
        // decided once; `resolved` holds the ones the walk continues through.
        let mut met = Vec::<BoundaryMachineId>::new();
        let mut resolved = Vec::<(BoundaryMachineId, MachineId)>::new();
        let mut requirements = Vec::<DemandedRequirement>::new();
        loop {
            let graph = terminal_installation::SynchronousCallGraph::new(module, None, None)
                .and_then(|graph| graph.with_boundary_calls(resolved.iter().copied()))
                .map_err(defect)?;
            let mut reached = Vec::new();
            for root in roots {
                reached.extend(graph.from_root(*root).map_err(defect)?.boundaries);
            }
            reached.sort_unstable();
            reached.dedup();
            let before = resolved.len();
            for boundary in reached {
                if met.contains(&boundary) {
                    continue;
                }
                met.push(boundary);
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
                if let Err(demand) = unplanned_boundary(module, requirement) {
                    refusals.push(refuse(target, &format!("{demand} `{}`", requirement.name)));
                    continue;
                }
                let supplies = build
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
                        selection: selection.clone(),
                    })
                    .collect::<Vec<_>>();
                let realization = chosen(&supplies).ok().and_then(|supply| {
                    let provider = supply.selection.provider_type.symbol;
                    judged
                        .iter()
                        .find(|row| {
                            row.provider == provider && row.requirement == requirement.requirement
                        })
                        .map(|row| row.machine)
                });
                if let Some(machine) = realization {
                    resolved.push((boundary, machine));
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
                    let mut paths = ranked.iter().map(|supply| {
                        format!("`{}`", supply.selection.provider_type.authored_path)
                    });
                    let first = paths.next().unwrap_or_default();
                    rejections.push(Diagnostic::error(format!(
                        "boundary requirement `{}` has conflicting providers on target `{target}`: {} declares {first} and {}",
                        reached.name,
                        match ranked[0].authority {
                            SupplyAuthority::Build => "the Build",
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
    /// The demand walk established that no supply is needed. A nonempty
    /// choice needs the canonical row realization operation instead.
    pub fn realize_no_demand(&self) -> Result<super::RealizedTargetSupply, Vec<Diagnostic>> {
        if !self.requirements.is_empty() {
            return Err(vec![Diagnostic::error(
                "nonempty provider supply needs its exact accepted module and execution inputs",
            )]);
        }
        super::RealizedTargetSupply::no_demand(self.target())
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    pub fn requirements(&self) -> &[SettledRequirement] {
        &self.requirements
    }

    /// Compatibility source choices can only request the plans already chosen
    /// and proved in this module. No choice or candidate is inferred here.
    pub fn realize(
        &self,
        planning: SupplyPlanning<'_>,
    ) -> Result<super::RealizedTargetSupply, Vec<Diagnostic>> {
        if self.requirements.is_empty() {
            return self.realize_no_demand();
        }
        let target = self.target.as_deref().ok_or_else(|| {
            vec![Diagnostic::error(
                "provider settlement lost its deployment target",
            )]
        })?;
        let module = planning.module;
        let refuse = |reason: &str| {
            vec![Diagnostic::error(format!(
                "provider settlement for `{target}`: {reason}",
            ))]
        };
        let mut reached = Vec::with_capacity(self.requirements.len());
        let mut diagnostic_choices = Vec::with_capacity(self.requirements.len());
        for requirement in &self.requirements {
            let selection = &requirement.supply.selection;
            let machine = requirement.realization.ok_or_else(|| {
                refuse(&format!(
                    "`{}` is supplied by `{}`, which the accepted module does not hold as a judged provider",
                    requirement.reached.name, selection.provider_type.authored_path,
                ))
            })?;
            let ProviderSelectionSubject::BoundaryTrait(slot) = &selection.subject else {
                return Err(refuse(
                    "provider plan needs its exact whole boundary trait selection",
                ));
            };
            let mut schemas = planning
                .schemas
                .iter()
                .filter(|schema| schema.slot == slot.symbol);
            let (Some(source), None) = (schemas.next(), schemas.next()) else {
                return Err(refuse(
                    "selected slot has no unique complete schema correspondence",
                ));
            };
            let mut schemas = module
                .service_schemas
                .iter()
                .filter(|schema| schema.surface == source.surface);
            let (Some(schema), None) = (schemas.next(), schemas.next()) else {
                return Err(refuse(
                    "selected slot schema has no unique published declaration application",
                ));
            };
            module
                .service_schema(schema.declaration, schema.application)
                .map_err(|reason| refuse(&reason))?;
            if slot.applied && slot.application_identity.is_none()
                || slot.application_identity.as_ref().is_some_and(|identity| {
                    module.identities.text(schema.application) != Some(identity.as_str())
                })
            {
                return Err(refuse(
                    "selected slot application differs from its published schema",
                ));
            }
            if module
                .boundary_machine(requirement.reached.boundary)
                .is_none()
                || module.machine(machine).is_none()
            {
                return Err(refuse(
                    "selected provider lost its actual boundary or machine",
                ));
            }
            let mut candidates = module.provider_candidates.iter().filter(|candidate| {
                candidate.boundary == requirement.reached.boundary && candidate.candidate == machine
            });
            let (Some(candidate), None) = (candidates.next(), candidates.next()) else {
                return Err(refuse(
                    "selected provider realization has no unique exact checked candidate",
                ));
            };
            let mut choices = module.provider_selections.iter().filter(|choice| {
                choice.target == target
                    && choice.schema_declaration == schema.declaration
                    && choice.schema_application == schema.application
                    && choice.provider_declaration == candidate.provider_declaration.identity()
                    && choice.provider_identity == candidate.provider_identity
            });
            let (Some(choice), None) = (choices.next(), choices.next()) else {
                return Err(refuse(
                    "selected provider has no unique prepublished target choice",
                ));
            };
            if choice.composition_mode != selection.composition_mode
                || choice.provider_package != selection.provider_type.package
                || selection.provider_type.applied
                    && selection.provider_type.application_identity.is_none()
                || choice
                    .provider_application
                    .and_then(|identity| module.identities.text(identity))
                    != selection.provider_type.application_identity.as_deref()
            {
                return Err(refuse(
                    "selected provider application, package or composition differs from its published choice",
                ));
            }
            diagnostic_choices.push((choice, selection.provider_type.authored_path.as_str()));
            reached.push(requirement.reached.boundary);
        }
        reached.sort_unstable();
        reached.dedup();
        super::plan_realization::realize_target_supply_detailed(
            super::TargetSupplyRealizationRequest::from_execution_inputs(
                module,
                target,
                &reached,
                planning.execution,
            ),
        )
        .map_err(|error| refuse(&realization_diagnostic(error, module, &diagnostic_choices)))
    }

    pub fn plans(
        &self,
        planning: SupplyPlanning<'_>,
    ) -> Result<Vec<backend_plan::TerminalTargetProviderPlan>, Vec<Diagnostic>> {
        self.realize(planning)
            .map(|realized| realized.provider_declarations())
    }
}

fn realization_diagnostic(
    error: super::plan_realization::TargetSupplyRealizationError,
    module: &TerminalModule,
    choices: &[(&terminal_psi::TerminalProviderSelection, &str)],
) -> String {
    use super::plan_realization::TargetSupplyRealizationError;

    let TargetSupplyRealizationError::MissingCheckedRow {
        provider_declaration,
        schema_declaration,
        schema_application,
        requirement_identity,
        ..
    } = &error
    else {
        return error.diagnostic(module);
    };
    let mut labels = choices.iter().filter(|(choice, _)| {
        choice.provider_declaration == *provider_declaration
            && choice.schema_declaration == *schema_declaration
            && choice.schema_application == *schema_application
    });
    let Some((_, provider_path)) = labels.next() else {
        return error.diagnostic(module);
    };
    if provider_path.is_empty() || labels.any(|(_, other)| other != provider_path) {
        return error.diagnostic(module);
    }
    let Ok(schema) = module.service_schema(*schema_declaration, *schema_application) else {
        return error.diagnostic(module);
    };
    let mut methods = schema.surface.methods.iter().filter(|method| {
        schema.surface.identity_text(method.requirement_identity) == requirement_identity.as_str()
    });
    let (Some(method), None) = (methods.next(), methods.next()) else {
        return error.diagnostic(module);
    };
    format!(
        "`{provider_path}` supplies `{}::{}` with no checked machine the accepted module states",
        method.requirement_owner, method.name,
    )
}

fn refuse(target: &str, demand: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "no product route yet: target preparation for `{target}`: the accepted module demands {demand}, which has no provider or calling plan on this route"
    ))
}

/// A selection names a requirement through the boundary trait that owns it,
/// through the requirement itself, or through an operator family holding it.
/// Declarations are compared by identity, never by spelling.
fn names_requirement(
    subject: &ProviderSelectionSubject,
    requirement: &ReachedBoundaryRequirement,
) -> bool {
    match subject {
        ProviderSelectionSubject::BoundaryTrait(identity) => identity.symbol == requirement.slot,
        ProviderSelectionSubject::BoundaryRequirement(identity) => {
            identity.symbol == requirement.requirement
        }
        ProviderSelectionSubject::BoundaryOperatorFamily(family) => family
            .coordinates()
            .iter()
            .any(|coordinate| coordinate.symbol == requirement.requirement),
    }
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
        (
            !module.provider_candidates.is_empty()
                || !module.provider_instance_requirements.is_empty(),
            "a provider instance",
        ),
        (
            !module.boundary_application_candidates.is_empty()
                || !module.boundary_application_demands.is_empty(),
            "an applied boundary requirement",
        ),
        (
            !module.calling_policy_requests.is_empty(),
            "a calling policy",
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
        (
            !module.target_machine_families.is_empty(),
            "a Build-selected target implementation",
        ),
    ]
    .into_iter()
    .filter_map(|(demanded, name)| demanded.then_some(name))
    .collect()
}

/// A reached requirement is plannable as a provider slot only when the
/// module declares it as an ordinary provider-executed requirement that
/// returns normally.
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
    if declaration.completion != semantic_vocabulary::BoundaryCompletion::NormalReturn {
        return Err("the externally completed boundary");
    }
    if declaration.declaration.as_ref().is_some_and(|callable| {
        callable.execution != terminal_psi::CheckedBoundaryExecution::Provider
    }) {
        return Err("the compiler-executed boundary");
    }
    Ok(())
}

/// Exact source schema correspondence; the full surface must already be in
/// the accepted module. The source slot is never used as executable identity.
#[derive(Debug, Clone)]
pub struct SlotSchema {
    pub slot: symbols::SymbolHandle,
    pub surface: std::sync::Arc<effects::provider_plan::ServiceSurface>,
}

pub struct SupplyPlanning<'a> {
    pub module: &'a TerminalModule,
    pub schemas: &'a [SlotSchema],
    pub execution: super::TargetSupplyExecutionInputs,
}
