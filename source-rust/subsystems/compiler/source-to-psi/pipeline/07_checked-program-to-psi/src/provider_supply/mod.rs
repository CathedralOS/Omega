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

use crate::provider_choice_inputs::ProviderSelection;
use crate::provider_choice_inputs::ProviderSelectionSubject;
use crate::provider_choice_inputs::TargetProviderDefaults;
use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::TerminalModule;

mod demand;
mod package_review;
pub use demand::SourceProviderFacts;
pub use source_provider_publication::{
    PublishedTargetProviders, SourceNativeBinding, SourcePackageReviewRequirements,
    SourceProviderDemand,
};

pub use psi_provider_selections::logical_reach::ClosedLogicalProviderReach;

impl crate::AcceptedTerminalSource {
    /// Finish logical body discovery without invoking a calling policy or
    /// inspecting a native plan. ABI failure cannot reopen this worklist.
    pub fn close_logical_provider_reach(
        &self,
        target: &str,
        entry: Option<MachineId>,
        selected_requirements: &[BoundaryMachineId],
    ) -> Result<ClosedLogicalProviderReach, Vec<Diagnostic>> {
        psi_provider_selections::logical_reach::close_providers(
            self.module(),
            target,
            entry,
            selected_requirements,
        )
    }
}

/// One boundary requirement the accepted module declares: the callable the
/// module declares for it, the source declaration of that requirement and of
/// the boundary trait that owns it (the two declarations a provider
/// selection can name), and the authored `Trait::requirement` spelling for
/// messages. The coordinator fills the declared rows from
/// `AcceptedTerminalSource::boundary_requirements_declared`; the rows a
/// prepared supply holds are the ones the target's entries reach.
pub use crate::DeclaredBoundaryRequirement as ReachedBoundaryRequirement;

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
    Native(crate::provider_choice_inputs::NativeBuildSupply),
}

impl ProviderSupply {
    pub fn checked(&self) -> Option<&ProviderSelection> {
        match &self.selection {
            SupplySelection::Checked(selection) => Some(selection),
            SupplySelection::Native(_) => None,
        }
    }

    /// How a diagnostic names this supply: its provider, or the native binding's producer.
    pub fn description(&self) -> String {
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
    pub source: &'a crate::source_state::TypedTrees,
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
    pub native_bindings: &'a [crate::provider_choice_inputs::NativeBuildSupply],
    /// The defaults target packages retain.
    pub defaults: &'a [TargetProviderDefaults],
    /// The selected deployment, or no deployment for targetless checking.
    pub target: Option<&'a str>,
    pub numeric_support: Option<&'a language_core::float_operations::NumericTargetCapabilities>,
}

impl PreparedTargetSupply {
    pub fn prepare(request: TargetSupplyRequest<'_>) -> Result<Self, Vec<Diagnostic>> {
        let TargetSupplyRequest {
            source,
            module,
            declared,
            roots,
            judged,
            build,
            native_bindings,
            defaults,
            target,
            numeric_support,
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
            .interfaces
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
        let selected_requirements = explicit_build_requirements(declared, build, target);
        let mut roots = roots.to_vec();
        // Every boundary the walk has met, demanded or refused, so each is
        // decided once; `resolved` holds the ones the walk continues through.
        let mut met = Vec::<BoundaryMachineId>::new();
        let mut resolved = Vec::<(BoundaryMachineId, MachineId)>::new();
        let mut requirements = Vec::<DemandedRequirement>::new();
        // The module does not change while the walk runs, so its machine
        // catalog and callable values are built once in these analyses; each
        // round only applies the boundaries it resolved to the shared values.
        let analyses =
            terminal_semantics::synchronous_calls::call_analyses(module).map_err(defect)?;
        let mut graph = analyses.calls();
        loop {
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
                            require_numeric_support(module, declaration, target, numeric_support)
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
                for supply in native_bindings.iter().filter(|supply| {
                    supply.selection.target.canonical_name() == target
                        && supply.selection.subject == requirement.slot
                }) {
                    match supply
                        .selection
                        .requirement_identity_for(source, requirement.requirement)
                    {
                        Ok(Some(_)) => supplies.push(ProviderSupply {
                            authority: SupplyAuthority::NativeBinding,
                            selection: SupplySelection::Native(supply.clone()),
                        }),
                        Ok(None) => {}
                        Err(reason) => refusals.push(refuse(target, &reason)),
                    }
                }
                if let Err(demand) = unplanned_boundary(module, requirement) {
                    refusals.push(refuse(target, &format!("{demand} `{}`", requirement.name)));
                    continue;
                }
                let realization = choose_supply(&supplies).ok().and_then(|supply| {
                    let provider = &supply.checked()?.provider_type;
                    judged
                        .iter()
                        .find(|row| {
                            row.provider == provider.symbol
                                && row.requirement == requirement.requirement
                                && provider
                                    .application_identity
                                    .as_ref()
                                    .is_none_or(|identity| {
                                        module.interfaces.provider_candidates.iter().any(
                                            |candidate| {
                                                candidate.boundary == boundary
                                                    && candidate.candidate == row.machine
                                                    && module
                                                        .identities
                                                        .text(candidate.provider_identity)
                                                        == Some(identity.as_str())
                                            },
                                        )
                                    })
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
            graph = graph
                .with_boundary_calls(resolved[before..].iter().copied())
                .map_err(defect)?;
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

    pub fn requirements(&self) -> &[DemandedRequirement] {
        &self.requirements
    }
}

/// Why a demanded requirement has no chosen supply.
#[derive(Debug)]
pub enum UnchosenSupply<'a> {
    Unsupplied,
    Competing(Vec<&'a ProviderSupply>),
}

/// The one supply at the highest authority that declared any.
pub fn choose_supply(supplies: &[ProviderSupply]) -> Result<&ProviderSupply, UnchosenSupply<'_>> {
    let authority = supplies
        .iter()
        .map(|supply| supply.authority)
        .max()
        .ok_or(UnchosenSupply::Unsupplied)?;
    let mut ranked = supplies
        .iter()
        .filter(|supply| supply.authority == authority);
    let first = ranked
        .next()
        .expect("the maximum authority belongs to a supply");
    match ranked.next() {
        None => Ok(first),
        Some(second) => Err(UnchosenSupply::Competing(
            [first, second].into_iter().chain(ranked).collect(),
        )),
    }
}

impl DemandedRequirement {
    /// The supply settlement would choose, when it would choose one.
    pub fn chosen(&self) -> Option<&ProviderSupply> {
        choose_supply(&self.supplies).ok()
    }
}

fn refuse(target: &str, demand: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "no product route yet: target preparation for `{target}`: the accepted module demands {demand}, which has no provider or calling plan on this route"
    ))
}

/// Exact explicit Build obligations for one deployment. Defaults and unselected
/// declarations contribute no roots. Applied selections cannot name an unapplied
/// declaration; indexed identities retain both arena coordinates and generations.
pub fn explicit_build_requirements(
    declared: &[ReachedBoundaryRequirement],
    build: &[ProviderSelection],
    target: Option<&str>,
) -> Vec<BoundaryMachineId> {
    let mut selections = build
        .iter()
        .filter(|selection| {
            selection
                .target
                .is_none_or(|selected| Some(selected.canonical_name()) == target)
        })
        .flat_map(|selection| selection_declarations(&selection.subject))
        .collect::<Vec<_>>();
    selections.sort_unstable();
    selections.dedup();
    let mut requirements = declared
        .iter()
        .filter(|requirement| {
            requirement
                .selection_declarations()
                .any(|key| selections.binary_search(&key).is_ok())
        })
        .map(|requirement| requirement.boundary)
        .collect::<Vec<_>>();
    requirements.sort_unstable();
    requirements.dedup();
    requirements
}

/// A selection names a requirement through the boundary trait that owns it,
/// through the requirement itself, or through an operator family holding it.
/// Declarations are compared by identity, never by spelling.
fn names_requirement(
    subject: &ProviderSelectionSubject,
    requirement: &ReachedBoundaryRequirement,
) -> bool {
    selection_declarations(subject).any(|selected| {
        requirement
            .selection_declarations()
            .any(|declared| declared == selected)
    })
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SelectionDeclarationKey<'a> {
    Slot(u32, u32, Option<&'a str>),
    Requirement(u32, u32, Option<&'a str>),
}

impl<'a> SelectionDeclarationKey<'a> {
    fn slot(symbol: symbols::SymbolHandle, application: Option<&'a str>) -> Self {
        Self::Slot(symbol.arena_index(), symbol.generation(), application)
    }

    fn requirement(symbol: symbols::SymbolHandle, application: Option<&'a str>) -> Self {
        Self::Requirement(symbol.arena_index(), symbol.generation(), application)
    }
}

impl ReachedBoundaryRequirement {
    fn selection_declarations(&self) -> impl Iterator<Item = SelectionDeclarationKey<'_>> {
        // A whole-trait choice covers its members. A choice for Reading<17>
        // covers only the members whose receiver has that same application.
        let declared = [
            SelectionDeclarationKey::slot(self.slot, None),
            SelectionDeclarationKey::requirement(self.requirement, None),
        ];
        let applied = self.application.as_ref().map(|application| {
            [
                SelectionDeclarationKey::slot(self.slot, Some(&application.receiving_trait)),
                SelectionDeclarationKey::requirement(
                    application.template,
                    Some(&application.callable),
                ),
            ]
        });
        declared.into_iter().chain(applied.into_iter().flatten())
    }
}

fn selection_declarations(
    subject: &ProviderSelectionSubject,
) -> impl Iterator<Item = SelectionDeclarationKey<'_>> {
    let (single, family) = match subject {
        ProviderSelectionSubject::BoundaryTrait(identity) => (
            Some(SelectionDeclarationKey::slot(
                identity.symbol,
                identity.application_identity.as_deref(),
            )),
            &[] as &[_],
        ),
        ProviderSelectionSubject::BoundaryRequirement(identity) => (
            Some(SelectionDeclarationKey::requirement(
                identity.symbol,
                identity.application_identity.as_deref(),
            )),
            &[] as &[_],
        ),
        ProviderSelectionSubject::BoundaryOperatorFamily(family) => (None, family.coordinates()),
    };
    single.into_iter().chain(
        family
            .iter()
            .map(|coordinate| SelectionDeclarationKey::requirement(coordinate.symbol, None)),
    )
}

/// The module-level demand tables whose rows need a plan this route cannot
/// form yet. Each is named for what a program asked for, and an empty table
/// asks for nothing. Checked candidate declarations are inventory: reached
/// choices join them through canonical realization and its actual execution
/// inputs. They introduce no instance demand of their own. Calling requests
/// record checked policy declarations; their presence creates no provider
/// demand. For an activated schema, execution preparation evaluates its policy
/// for the exact application and target before realization uses the plan.
fn unplanned_demands(module: &TerminalModule) -> Vec<&'static str> {
    [
        (
            !module.interfaces.external_providers.is_empty(),
            "an external provider realization",
        ),
        (
            !module.interfaces.provider_instance_requirements.is_empty(),
            "a provider instance",
        ),
        (
            !module.interfaces.boundary_application_candidates.is_empty()
                || !module.interfaces.boundary_application_demands.is_empty(),
            "an applied boundary requirement",
        ),
        (
            !module.interfaces.callback_applications.is_empty()
                || !module
                    .interfaces
                    .installed_interrupt_registrations
                    .is_empty(),
            "a callback",
        ),
        (
            !module.execution.task_activation_demands.is_empty(),
            "a task activation",
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
    if declaration.declaration.as_ref().is_some_and(|callable| {
        callable.execution != terminal_psi::CheckedBoundaryExecution::Provider
    }) {
        return Err("the compiler-executed boundary");
    }
    Ok(())
}

/// This classification consumes an already checked declaration; it grants no
/// native execution. Target realization independently rejoins its core link.
pub(crate) fn is_direct_compiler_primitive(
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> bool {
    boundary.declaration.as_ref().is_some_and(|declaration| {
        matches!(
            declaration.execution,
            terminal_psi::CheckedBoundaryExecution::HostedPrimitive { .. }
                | terminal_psi::CheckedBoundaryExecution::NumericPrimitive
                | terminal_psi::CheckedBoundaryExecution::LayoutTargetFacts
        )
    })
}

// Source closure observes the target's declared support only. It never obtains
// a register, calling convention or native plan from this observation.
fn require_numeric_support(
    module: &TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    target: &str,
    support: Option<&language_core::float_operations::NumericTargetCapabilities>,
) -> Result<(), Vec<Diagnostic>> {
    let fail = |reason: &str| vec![Diagnostic::error(reason)];
    if boundary.numeric_intrinsic.is_none() {
        return Err(fail("numeric primitive lost its semantic meaning"));
    }
    let support = support
        .filter(|support| support.target == target)
        .ok_or_else(|| fail("numeric primitive has no recognized target"))?;
    let requirement = module
        .identities
        .text(boundary.identity)
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| fail("numeric primitive lost its exact declaration identity"))?;
    let support = support
        .support_for(requirement)
        .ok_or_else(|| fail("numeric primitive has no canonical target realization entry"))?;
    support.map_err(|constraint| vec![Diagnostic::error(format!(
        "canonical float operation `{requirement}` has no realization on target `{target}`: {constraint}"
    ))])
}

#[cfg(test)]
mod tests;
