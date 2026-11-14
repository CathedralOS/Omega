//! What one target must supply for a Product, decided by what the Product's
//! accepted Terminal module demands.
//!
//! A program needs a provider for a boundary requirement only when its entry
//! reaches that requirement, and the accepted module says which ones it
//! reaches: it declares a boundary callable exactly for the requirements its
//! machines call. Target preparation therefore reads the demand from the
//! module and pairs each demanded requirement with the supply declared for
//! the target: the provider selections the evaluated Build made and the
//! defaults the target package retains. Both are the same `ProviderSelection`;
//! only the authority that declared them differs, and settlement ranks on
//! that.
//!
//! A module that demands nothing prepares an empty supply, and that is a
//! finding about the artifact rather than a skipped stage. A module that
//! demands a kind of supply nothing here can plan yet is refused by the name
//! of what it demands, so the refusal reaches only programs that need it.
//!
//! Preparation does not choose. Settlement does: each demanded requirement
//! must have exactly one supply at the highest authority that declared any,
//! and a requirement with none or with several rejects the program
//! (wiki/spec/build/provider_selection.md, "Missing, duplicate, incompatible,
//! or unadmitted selected supply rejects rather than choosing a body by
//! spelling or source order"). The chosen supply then needs its provider plan
//! checked against the slot; that plan has no producer on this route yet, so
//! `SettledTargetSupply::plans` refuses each chosen supply by name.

use build_time_evaluation::provider_declarations::{
    ProviderSelection, ProviderSelectionSubject, TargetProviderDefaults,
};
use diagnostics::Diagnostic;
use terminal_psi::TerminalModule;

/// One boundary requirement the accepted module's machines call: the
/// callable the module declares for it, the source declaration of that
/// requirement and of the boundary trait that owns it (the two declarations
/// a provider selection can name), and the authored `Trait::requirement`
/// spelling for messages. The coordinator fills it from
/// `AcceptedTerminalSource::boundary_requirements_reached`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachedBoundaryRequirement {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub requirement: symbols::SymbolHandle,
    pub slot: symbols::SymbolHandle,
    pub name: String,
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

/// One boundary requirement the module reaches and every supply declared for
/// it on this target, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandedRequirement {
    pub reached: ReachedBoundaryRequirement,
    pub supplies: Vec<ProviderSupply>,
}

/// The demand of one accepted module, retaining an optional selected deployment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTargetSupply {
    target: Option<String>,
    requirements: Vec<DemandedRequirement>,
}

pub struct TargetSupplyRequest<'a> {
    /// The accepted module and the requirements it reaches
    /// (`AcceptedTerminalSource::{module, boundary_requirements_reached}`).
    pub module: &'a TerminalModule,
    pub reached: &'a [ReachedBoundaryRequirement],
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
            reached,
            build,
            defaults,
            target,
        } = request;
        let mut refusals = unplanned_demands(module)
            .into_iter()
            .map(|demand| refuse(target.unwrap_or("<unscoped>"), demand))
            .collect::<Vec<_>>();
        let mut requirements = Vec::with_capacity(reached.len());
        for requirement in reached {
            let Some(target) = target else {
                refusals.push(Diagnostic::error(format!(
                    "boundary requirement `{}` requires an explicit deployment target",
                    requirement.name
                )));
                continue;
            };
            match unplanned_boundary(module, requirement) {
                Ok(()) => {}
                Err(demand) => {
                    refusals.push(refuse(target, &format!("{demand} `{}`", requirement.name)));
                    continue;
                }
            }
            let declared = build
                .iter()
                .map(|selection| (SupplyAuthority::Build, selection))
                .chain(
                    defaults
                        .iter()
                        .filter(|row| row.target.as_deref().is_none_or(|row| row == target))
                        .flat_map(|row| &row.selections)
                        .map(|selection| (SupplyAuthority::TargetDefault, selection)),
                );
            let supplies = declared
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
                .collect();
            requirements.push(DemandedRequirement {
                reached: requirement.clone(),
                supplies,
            });
        }
        if module.boundary_machines.len() != reached.len() {
            refusals.push(refuse(
                target.unwrap_or("<unscoped>"),
                "boundary declarations that differ from the requirements it reaches",
            ));
        }
        if !refusals.is_empty() {
            return Err(refusals);
        }
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

    /// Whether the module reaches no boundary requirement, so this target
    /// has no provider to settle.
    pub fn demands_nothing(&self) -> bool {
        self.requirements.is_empty()
    }
}

/// One demanded requirement and the single supply chosen for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledRequirement {
    pub reached: ReachedBoundaryRequirement,
    pub supply: ProviderSupply,
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
        for DemandedRequirement { reached, supplies } in requirements {
            let target = target.as_deref().ok_or_else(|| {
                vec![Diagnostic::error(
                    "provider settlement lost its deployment target",
                )]
            })?;
            let Some(authority) = supplies.iter().map(|supply| supply.authority).max() else {
                rejections.push(Diagnostic::error(format!(
                    "boundary requirement `{}` has no provider on target `{target}`: the Build selects none and the target package declares no default",
                    reached.name
                )));
                continue;
            };
            let mut ranked = supplies
                .into_iter()
                .filter(|supply| supply.authority == authority);
            let Some(supply) = ranked.next() else {
                continue;
            };
            let competing = ranked
                .map(|other| format!("`{}`", other.selection.provider_type.authored_path))
                .collect::<Vec<_>>();
            if !competing.is_empty() {
                rejections.push(Diagnostic::error(format!(
                    "boundary requirement `{}` has conflicting providers on target `{target}`: {} declares `{}` and {}",
                    reached.name,
                    match authority {
                        SupplyAuthority::Build => "the Build",
                        SupplyAuthority::TargetDefault => "the target package",
                    },
                    supply.selection.provider_type.authored_path,
                    competing.join(" and "),
                )));
                continue;
            }
            settled.push(SettledRequirement { reached, supply });
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

    /// The provider plans of the chosen supplies: none for a module that
    /// demands nothing. A chosen supply needs its provider checked against
    /// the slot and a calling plan for the selected target; neither has a
    /// producer on this route, so each is refused by name.
    pub fn plans(&self) -> Result<Vec<effects::provider_plan::ProviderPlan>, Vec<Diagnostic>> {
        if self.requirements.is_empty() {
            return Ok(Vec::new());
        }
        Err(self
            .requirements
            .iter()
            .map(|requirement| {
                Diagnostic::error(format!(
                    "no product route yet: provider settlement for `{}`: `{}` is supplied by `{}`, and no provider or calling plan can be formed for a selected provider on this route",
                    self.target.as_deref().unwrap_or("<unscoped>"),
                    requirement.reached.name,
                    requirement.supply.selection.provider_type.authored_path,
                ))
            })
            .collect())
    }
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
