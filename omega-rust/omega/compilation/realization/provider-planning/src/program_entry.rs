/*
We select the target-qualified `ProgramEntry` root slot and validate both its
source contract and its physical calling realization.

Entry slots select a contract owner before selecting declarations: bundled
contracts use their exact source custody, while accepted packages use their
explicit package/declaration binding. The physical requirement comes from that
schema's resolved composition graph; the semantic requirement retains `core`'s
identity. Calling plans must join both exact requirement symbols to the
selected schema so unrelated same-leaf traits cannot redirect selection, even
when Build and Product retain separate package instances.
*/

/*
A single `Build` declaration may describe a target matrix: well-formed slots
owned by other known profiles are validated for source-shape compatibility and
left for those profiles, while `selected_program_entry_machine` consumes only
the chosen profile's exact row. When a root binding names a target-scoped
machine family rather than a single body, we resolve the exact family member
for the target profile without altering the shared source graph.
*/

/*
We separate source-contract preparation (`prepare_compiler_program_entry`) from
ABI calling-plan settlement (`PreparedCompilerProgramEntry::settle_calling_plans`).
`extent_layout::capture` derives the address-free `Extent` record layout (`base:
addr; length: u64`) for the UEFI/Microsoft program-storage schema during source
preparation, while `calling_plans::settle` joins the evaluated semantic and
physical `BoundaryCallingPlanRealization` rows after calling policies run,
without reopening any source context.
*/

use crate::provider_planning::entry_state;
use build_time_evaluation::product_entries::{
    validate_source_compatibility, visible_entry_parameters,
};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::callable_signature_shapes::{
    BoundCallableShape, ParameterOccurrence,
};
use checked_trees::typed_trees::product_operand::{
    ProductEntryDeclaration, core_arrival_requirement,
};
use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use diagnostics::Diagnostic;
pub use language_semantics::declaration_selection::RootBinding;
pub use package_compilation::program_entry_semantic_binding_role;

mod discovery;
mod source_contract;
pub use discovery::discover_explicit_entry_binding;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedProgramEntry<'config> {
    pub machine_name: &'config str,
    pub machine_symbol: symbols::SymbolHandle, // Exact machine selected under the binding occurrence's lexical package.
    pub slot: target::ProgramEntrySlotDeclaration,
}

/// Retain every executed binding's visible calling shape, including rows for
/// targets this invocation does not realize. Source custody names the core
/// semantic arrival requirement; physical admission stays with the selected
/// target, so an inactive row does not demand its package acceptance. The physical schema,
/// accepted package digest, backing and effect/resource checks still belong to
/// their existing admission paths; a shape receipt grants none of those.
pub fn bound_program_entry_shapes(
    typed: &TypedTrees,
    bindings: &[RootBinding],
) -> Result<Vec<BoundCallableShape>, Vec<Diagnostic>> {
    let mut shapes = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let resolved = (|| -> Result<_, Diagnostic> {
            let (owner, name) = binding
                .slot
                .rsplit_once("::")
                .ok_or_else(|| Diagnostic::error("root binding has no target-qualified slot"))?;
            let profile = target::TargetProfile::from_root_slot_owner(owner)
                .map_err(|_| Diagnostic::error(format!("unknown root slot owner `{owner}`")))?;
            let slot = profile
                .required_root_slot(name)
                .and_then(|slot| slot.program_entry())
                .ok_or_else(|| {
                    Diagnostic::error(format!("unsupported program root slot `{}`", binding.slot))
                })?;
            let (requirement_owner, requirement) =
                core_arrival_requirement(typed, slot.semantic_arrival_requirement)
                    .map_err(Diagnostic::error)?;
            let visible_parameters = match slot.source_contract().visible_parameters {
                semantic_vocabulary::ProgramEntryVisibleParameters::None => Vec::new(),
                semantic_vocabulary::ProgramEntryVisibleParameters::ImageAndInitialStorage => typed
                    .state_signature_parameters(requirement)
                    .iter()
                    .enumerate()
                    .map(|(ordinal, parameter)| ParameterOccurrence {
                        ordinal,
                        symbol: parameter.symbol,
                    })
                    .collect(),
            };
            let mut implementation = binding.implementation_symbol;
            if let Some(family) = typed.target_machine_families.iter().find(|family| {
                family.canonical == implementation
                    || family
                        .members
                        .iter()
                        .any(|member| member.machine == implementation)
            }) {
                implementation = family
                    .members
                    .iter()
                    .find(|member| member.target.as_str() == profile.target_name())
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "entry family `{}` has no body for target `{}`",
                            family.name,
                            profile.target_name()
                        ))
                    })?
                    .machine;
            }
            Ok(BoundCallableShape {
                slot: binding.slot.clone(),
                arrival_description: match slot.source_contract().visible_parameters {
                    semantic_vocabulary::ProgramEntryVisibleParameters::None => {
                        "hosted `ProgramEntry` exposes no arrival parameters"
                    }
                    semantic_vocabulary::ProgramEntryVisibleParameters::ImageAndInitialStorage => {
                        "target schema exposes exactly image and initial-storage roots"
                    }
                },
                source_span: binding.source_span,
                implementation,
                semantic_schema_symbol: requirement_owner.symbol,
                requirement_symbol: requirement.symbol,
                semantic_schema: symbols::RetainedDeclaration::capture(
                    &typed.symbols,
                    requirement_owner.symbol,
                )
                .map_err(Diagnostic::error)?,
                requirement: symbols::RetainedDeclaration::capture(
                    &typed.symbols,
                    requirement.symbol,
                )
                .map_err(Diagnostic::error)?,
                visible_parameters,
            })
        })();
        match resolved {
            Ok(shape) => shapes.push(shape),
            Err(mut diagnostic) => {
                if let Some(span) = binding.source_span {
                    diagnostic = diagnostic.with_source_span(span);
                }
                return Err(vec![diagnostic]);
            }
        }
    }
    Ok(shapes)
}

pub fn selected_program_entry_machine<'config>(
    root_bindings: &'config [RootBinding],
    selected_profile: Option<target::TargetProfile>,
) -> Result<Option<SelectedProgramEntry<'config>>, Vec<Diagnostic>> {
    let Some(selected_profile) = selected_profile else {
        return Ok(None);
    };
    if root_bindings.is_empty() {
        return Ok(None);
    }
    let mut diagnostics = Vec::new();
    let mut selected_bindings = Vec::new();
    for binding in root_bindings {
        let Some((profile, slot_name)) = binding.slot.rsplit_once("::") else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}` is not target-qualified; expected `target::ProgramEntry`",
                binding.slot
            )));
            continue;
        };
        let profile = match target::TargetProfile::from_root_slot_owner(profile) {
            Ok(profile) => profile,
            Err(_) => {
                diagnostics.push(Diagnostic::error(format!(
                    "root slot `{}` belongs to unknown target profile `{profile}`",
                    binding.slot
                )));
                continue;
            }
        };
        let Some(required_slot) = profile.required_root_slot(slot_name) else {
            diagnostics.push(Diagnostic::error(format!(
                "target profile `{}` declares no required root slot `{}`",
                profile.target_name(),
                binding.slot
            )));
            continue;
        };
        if profile != selected_profile {
            continue;
        }
        selected_bindings.push((required_slot, binding));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut program_entries = Vec::new();
    for required_slot in selected_profile.required_root_slots() {
        let matches = selected_bindings
            .iter()
            .filter(|(selected, _)| selected == &required_slot)
            .collect::<Vec<_>>();
        let binding = match matches.as_slice() {
            [(_, binding)] => *binding,
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has no bound required root slot `{}::{}`",
                    selected_profile.target_name(),
                    required_slot.owner().root_slot_owner_name(),
                    required_slot.slot_name()
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has more than one bound required root slot `{}::{}`",
                    selected_profile.target_name(),
                    required_slot.owner().root_slot_owner_name(),
                    required_slot.slot_name()
                )));
                continue;
            }
        };
        let Some(program_entry) = required_slot.program_entry() else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}::{}` uses a target-required schema that the ProgramEntry source-lowering path does not implement",
                required_slot.owner().root_slot_owner_name(),
                required_slot.slot_name()
            )));
            continue;
        };
        program_entries.push((program_entry, binding));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    match program_entries.as_slice() {
        [(slot, binding)] => Ok(Some(SelectedProgramEntry {
            machine_name: binding.implementation.as_str(),
            machine_symbol: binding.implementation_symbol,
            slot: *slot,
        })),
        [] => Err(vec![Diagnostic::error(format!(
            "selected target `{}` has no supported ProgramEntry root schema",
            selected_profile.target_name()
        ))]),
        _ => Err(vec![Diagnostic::error(format!(
            "selected target `{}` declares more than one ProgramEntry root schema",
            selected_profile.target_name()
        ))]),
    }
}

fn validate_selected_program_entry_shape(
    typed: &TypedTrees,
    selected: SelectedProgramEntry<'_>,
) -> Result<SelectedEntryDeclaration, Vec<Diagnostic>> {
    let machine_name = selected.machine_name;
    let machine = typed.machine(selected.machine_symbol).ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "build root slot selected entry `{machine_name}` is not a declaration in the admitted program"
        ))]
    })?;
    let entry = entry_state(typed, machine).ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "entry machine `{machine_name}` has no executable entry state"
        ))]
    })?;
    validate_source_compatibility(
        typed,
        ProductEntryDeclaration::from_machine(typed, machine),
        selected.slot.source_contract(),
    )?;
    let parameters = typed.state_parameters(entry);
    let visible = visible_entry_parameters(typed, parameters);
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let visible_parameters = visible
        .iter()
        .enumerate()
        .map(|(index, parameter)| -> Result<_, Diagnostic> {
            let role = match index {
                0 => program_entry_plan::ProgramStorageEntryRootRole::Image,
                1 => program_entry_plan::ProgramStorageEntryRootRole::InitialStorage,
                _ => unreachable!("selected source shape validation fixed visible arity"),
            };
            let extent_value_layout =
                extent_layout::capture(typed, selected.slot, parameter.type_reference).map_err(
                    |diagnostic| {
                        Diagnostic::error(format!(
                            "selected entry machine `{machine_name}` visible parameter {index} has no exact Extent value layout: {diagnostic}"
                        ))
                    },
                )?;
            let value_shape = extent_value_layout.shape();
            Ok(program_entry_plan::SelectedProgramEntrySignature::visible_parameter(
                role,
                index,
                identities.intern(&typed.normalized_type_identity_text(parameter.type_reference)),
                value_shape,
                extent_value_layout,
                parameter.is_const,
                parameter.is_mutable,
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|diagnostic| vec![diagnostic])?;
    if !machine.symbol.is_valid() || !entry.symbol.is_valid() {
        return Err(vec![Diagnostic::error(
            "selected source signature contains an invalid declaration coordinate",
        )]);
    }
    let declaration_source = typed
        .symbols
        .symbol_provenance_source_span(machine.symbol)
        .map(|span| span.source_id);
    let exact_sources = package_compilation::toolchain_source_identities(
        typed
            .symbols
            .source_files()
            .filter(|file| Some(file.source_id) == declaration_source),
    )?;
    let declaration = typed
        .exact_owner_nominal_identity(machine.symbol, &exact_sources)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "selected program entry has no exact declaration owner",
            )]
        })?;
    let declaration = typed.type_identity_text(declaration).unwrap_or_default();
    let signature = program_entry_plan::SelectedProgramEntrySignature::from_checked_declaration(
        selected.slot,
        declaration,
        machine.name.as_str().to_owned(),
        entry.name.as_str().to_owned(),
        identities.intern(
            &typed
                .machine_callable_identity_text(machine)
                .expect("selected entry has one checked executable state"),
        ),
        visible_parameters,
        identities.build(),
    )
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic)])?;
    Ok(SelectedEntryDeclaration {
        machine: machine.symbol,
        state: entry.symbol,
        signature,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedEntryDeclaration {
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    signature: program_entry_plan::SelectedProgramEntrySignature,
}

pub struct PreparedCompilerProgramEntry {
    declaration: SelectedEntryDeclaration,
    slot: target::ProgramEntrySlotDeclaration,
    calling_contract:
        Result<Option<source_contract::PreparedEntryCallingContract>, Vec<Diagnostic>>,
}

impl PreparedCompilerProgramEntry {
    pub fn service_requirements(
        &self,
        checked: &checked_trees::CheckedTrees,
    ) -> Vec<symbols::SymbolHandle> {
        let reaches = &checked.facts.service_reaches;
        let Some(reach) = reaches.for_machine(self.declaration.machine) else {
            return Vec::new();
        };
        let published = match reach.interface {
            language_semantics::ServiceReachInterface::PublishedCeiling(row) => Some(row),
            language_semantics::ServiceReachInterface::InternalInferred => None,
        };
        published
            .into_iter()
            .chain([reach.inferred_transitive, reach.concrete_transitive])
            .flat_map(|row| reaches.rows.services(row))
            .filter_map(|&service| reaches.services.definition(service))
            .map(|definition| definition.symbol)
            .collect()
    }

    pub fn source_signature(&self) -> &program_entry_plan::SelectedProgramEntrySignature {
        &self.declaration.signature
    }

    pub fn settle_calling_plans(
        self,
        realizations: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    ) -> Result<SelectedCompilerProgramEntry, Vec<Diagnostic>> {
        let calling_plans = match self.calling_contract? {
            Some(contract) => Some(calling_plans::settle(self.slot, contract, realizations)?),
            None => None,
        };
        Ok(SelectedCompilerProgramEntry::new(
            self.declaration,
            calling_plans,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramEntryCallingPlans {
    pub semantic_calling_application: calling_conventions::BoundaryCallingApplication,
    pub physical_calling_application: calling_conventions::BoundaryCallingApplication,
    pub storage_entry: program_entry_plan::SelectedProgramStorageEntryPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCompilerProgramEntry {
    declaration: SelectedEntryDeclaration,
    calling_plans: Option<SelectedProgramEntryCallingPlans>,
    physical_adapter: Option<terminal_psi::PreparedNominalRoot>,
}

impl SelectedCompilerProgramEntry {
    fn new(
        declaration: SelectedEntryDeclaration,
        calling_plans: Option<SelectedProgramEntryCallingPlans>,
    ) -> Self {
        Self {
            declaration,
            calling_plans,
            physical_adapter: None,
        }
    }

    pub fn machine_name(&self) -> &str {
        self.declaration.signature.machine_name()
    }

    pub fn physical_adapter(&self) -> Option<&terminal_psi::PreparedNominalRoot> {
        self.physical_adapter.as_ref()
    }

    pub fn bind_physical_adapter(
        &mut self,
        module: &terminal_psi::TerminalModule,
        providers: &[crate::SelectedProviderReviewProvenance],
        declarations: &[backend_plan::TerminalTargetProviderPlan],
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(plans) = &self.calling_plans else {
            return Ok(());
        };
        let contract = plans.storage_entry.physical_contract().ok_or_else(|| {
            vec![Diagnostic::error(
                "selected entry lost its physical contract",
            )]
        })?;
        let requirement = contract
            .identities()
            .text(contract.requirement_identity())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "physical contract lost its requirement identity",
                )]
            })?;
        let mut selected = None;
        for provider in providers {
            for row in &provider.plan.rows {
                if provider.plan.identity_text(row.requirement_identity) != requirement {
                    continue;
                }
                if matches!(
                    provider.selected_by,
                    crate::ProviderSelectionProvenance::UniqueCoveringCandidate
                ) {
                    return Err(vec![Diagnostic::error(
                        "physical entry adapter requires an explicit provider selection",
                    )]);
                }
                if !matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
                ) {
                    return Err(vec![Diagnostic::error(
                        "selected physical entry adapter is absent",
                    )]);
                }
                let header =
                    backend_plan::selected_checked_adapter_header(&provider.plan, row, module)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                if !header.supplies_checked_body || header.type_parameter_count != 0 {
                    return Err(vec![Diagnostic::error(
                        "physical entry adapter must have a checked source body",
                    )]);
                }
                let mut matching = declarations
                    .iter()
                    .filter(|declaration| declaration.plan == provider.plan);
                let declaration = matching.next().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "physical entry adapter lost its exact selected declaration",
                    )]
                })?;
                if matching.next().is_some() {
                    return Err(vec![Diagnostic::error(
                        "physical entry adapter has competing selected declarations",
                    )]);
                }
                let owner = declaration.provider_declaration.as_deref().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "physical entry adapter lost its selecting owner",
                    )]
                })?;
                let rows =
                    backend_plan::selected_provider_candidate_rows(&provider.plan, owner, module)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                let candidates = rows
                    .iter()
                    .find(|candidate| candidate.row == row)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "physical entry adapter lost its exact requirement row",
                        )]
                    })?;
                let [position] = candidates.candidate_positions.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "physical entry requirement has {} canonical realizations instead of one",
                        candidates.candidate_positions.len(),
                    ))]);
                };
                let candidate = &module.provider_candidates[*position];
                let machine = module.machine(candidate.candidate).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected physical entry adapter is absent",
                    )]
                })?;
                let declaration = machine
                    .declaration
                    .as_ref()
                    .and_then(|declaration| module.identities.text(declaration.machine.identity()))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "physical entry adapter lost its nominal declaration",
                        )]
                    })?;
                let root = terminal_psi::PreparedNominalRoot::new(declaration, "", None)
                    .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
                if selected.replace(root).is_some() {
                    return Err(vec![Diagnostic::error(
                        "physical arrival has more than one selected adapter",
                    )]);
                }
            }
        }
        self.physical_adapter = selected;
        Ok(())
    }

    pub const fn source_signature(&self) -> &program_entry_plan::SelectedProgramEntrySignature {
        &self.declaration.signature
    }

    pub const fn machine_symbol(&self) -> symbols::SymbolHandle {
        self.declaration.machine
    }

    pub const fn state_symbol(&self) -> symbols::SymbolHandle {
        self.declaration.state
    }

    pub const fn calling_plans(&self) -> Option<&SelectedProgramEntryCallingPlans> {
        self.calling_plans.as_ref()
    }
}

fn check_unrequested_program_entry_rows(
    typed: &TypedTrees,
    root_bindings: &[RootBinding],
    selected_profile: Option<target::TargetProfile>,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for binding in root_bindings {
        let Some((owner, slot_name)) = binding.slot.rsplit_once("::") else {
            continue;
        };
        let Ok(profile) = target::TargetProfile::from_root_slot_owner(owner) else {
            continue;
        };
        if Some(profile) == selected_profile {
            continue;
        }
        let Some(required_slot) = profile.required_root_slot(slot_name) else {
            continue;
        };
        let Some(slot) = required_slot.program_entry() else {
            continue;
        };
        let mut machine_symbol = binding.implementation_symbol;
        if let Some(family) = typed.target_machine_families.iter().find(|family| {
            family.canonical == machine_symbol
                || family
                    .members
                    .iter()
                    .any(|member| member.machine == machine_symbol)
        }) {
            let Some(member) = family
                .members
                .iter()
                .find(|member| member.target.as_str() == profile.target_name())
            else {
                continue;
            };
            machine_symbol = member.machine;
        }
        if let Err(mut rejected) = validate_selected_program_entry_shape(
            typed,
            SelectedProgramEntry {
                machine_name: binding.implementation.as_str(),
                machine_symbol,
                slot,
            },
        ) {
            diagnostics.append(&mut rejected);
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub fn prepare_compiler_program_entry(
    services: ServiceCatalog<'_>,
    root_bindings: &[RootBinding],
    selected_profile: Option<target::TargetProfile>,
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<Option<PreparedCompilerProgramEntry>, Vec<Diagnostic>> {
    require_product_services(services)?;
    let typed = services.context().typed;
    let selected = selected_program_entry_machine(root_bindings, selected_profile)?;
    check_unrequested_program_entry_rows(typed, root_bindings, selected_profile)?;
    let Some(mut selected) = selected else {
        return Ok(None);
    };
    if let Some(family) = typed.target_machine_families.iter().find(|family| {
        family.canonical == selected.machine_symbol
            || family
                .members
                .iter()
                .any(|member| member.machine == selected.machine_symbol)
    }) {
        let profile = selected.slot.owner;
        let member = family
            .members
            .iter()
            .find(|member| member.target.as_str() == profile.target_name())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "entry family `{}` has no body for target `{}`",
                    family.name,
                    profile.target_name(),
                ))]
            })?;
        selected.machine_symbol = member.machine;
    }
    let declaration = validate_selected_program_entry_shape(typed, selected)?;
    let accepted_binding = selected
        .slot
        .physical_contract_package
        .map(program_entry_semantic_binding_role)
        .and_then(|role| package_inputs.and_then(|inputs| inputs.accepted_semantic_binding(role)));
    let calling_contract = source_contract::prepare(services, selected, accepted_binding);
    Ok(Some(PreparedCompilerProgramEntry {
        declaration,
        slot: selected.slot,
        calling_contract,
    }))
}

fn require_product_services(services: ServiceCatalog<'_>) -> Result<(), Vec<Diagnostic>> {
    if services.context().scope == source::DependencyScope::Product {
        Ok(())
    } else {
        Err(vec![Diagnostic::error(
            "program-entry selection requires the Product service catalog",
        )])
    }
}

mod calling_plans {
    use super::SelectedProgramEntryCallingPlans;
    use super::source_contract::{PreparedEntryCallingContract, PreparedEntrySurface};
    use diagnostics::Diagnostic;

    pub(super) fn settle(
        slot: target::ProgramEntrySlotDeclaration,
        contract: PreparedEntryCallingContract,
        realizations: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    ) -> Result<SelectedProgramEntryCallingPlans, Vec<Diagnostic>> {
        let PreparedEntryCallingContract {
            schema,
            schema_name,
            semantic_signature,
            semantic_identity,
            physical_signature,
            physical_identity,
            physical_requirement,
            physical_convention,
            semantic_convention,
            surface,
            parameter_type_identities,
            result_type_identity,
            accepted_binding,
        } = contract;
        let semantic_matching = realizations
            .iter()
            .filter(|realization| {
                realization.boundary_trait == schema
                    && realization.requirement_machine == semantic_signature
            })
            .collect::<Vec<_>>();
        let [semantic_realization] = semantic_matching.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` retains {} evaluated calling plans for semantic requirement `{}` instead of exactly one",
                semantic_matching.len(),
                slot.semantic_arrival_requirement,
            ))]);
        };
        let physical_matching = realizations
            .iter()
            .filter(|realization| {
                realization.boundary_trait == schema
                    && realization.requirement_machine == physical_signature
            })
            .collect::<Vec<_>>();
        let [physical_realization] = physical_matching.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` retains {} evaluated calling plans for physical requirement `{physical_requirement}` instead of exactly one",
                physical_matching.len(),
            ))]);
        };
        for (role, realization) in [
            ("semantic", *semantic_realization),
            ("physical", *physical_realization),
        ] {
            let (validated, application_report_fingerprint, application_commitment) = realization
                .replayed_validated_application()
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "target boundary schema `{schema_name}` retains an invalid {role} calling plan: {error}"
                    ))]
                })?;
            if realization.exact_boundary_entry_plan() != validated.plan()
                || realization.report_fingerprint != application_report_fingerprint
                || realization.commitment != application_commitment
            {
                return Err(vec![Diagnostic::error(format!(
                    "target boundary schema `{schema_name}` does not retain exact {role} calling-plan identity and custody"
                ))]);
            }
        }
        let (validated_physical_plan, _, _) = physical_realization
            .replayed_validated_application()
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "target boundary schema `{schema_name}` lost its validated physical calling plan: {error}"
                ))]
            })?;
        let expected_physical =
            calling_conventions::CallingPolicy::for_program_entry_convention(physical_convention);
        if physical_realization.boundary_entry_plan.call.policy != expected_physical {
            return Err(vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` evaluates physical requirement `{physical_requirement}` with {:?}, but `{}::{}` requires {:?}",
                physical_realization.boundary_entry_plan.call.policy,
                slot.owner.root_slot_owner_name(),
                slot.slot_name,
                expected_physical,
            ))]);
        }
        let expected_semantic =
            calling_conventions::CallingPolicy::for_program_entry_convention(semantic_convention);
        if semantic_realization.boundary_entry_plan.call.policy != expected_semantic {
            return Err(vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` evaluates semantic requirement `{}` with {:?}, but `{}::{}` requires {:?}",
                slot.semantic_arrival_requirement,
                semantic_realization.boundary_entry_plan.call.policy,
                slot.owner.root_slot_owner_name(),
                slot.slot_name,
                expected_semantic,
            ))]);
        }
        let PreparedEntrySurface {
            declaration,
            physical_source,
        } = surface?;
        let prepared = declaration
            .surface()
            .map_err(|diagnostic| vec![diagnostic])?;
        let service_schema = crate::service_schema::attach_calling(&prepared, realizations);
        let physical_source = physical_source?
            .validate(
                &service_schema,
                &physical_identity,
                accepted_binding.as_ref(),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        let storage_entry = program_entry_plan::SelectedProgramStorageEntryPlan::from_target_slot(
            slot,
            service_schema,
            semantic_identity,
        )
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
        let result_type_identity = result_type_identity.ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "physical entry requirement `{physical_requirement}` has no result"
            ))]
        })?;
        let physical_contract = program_entry_plan::ProgramEntryPhysicalContractPlan::new(
            slot,
            physical_identity,
            physical_source.package,
            physical_source.package_source_digest,
            physical_source.non_authoritative_package_source_report_fingerprint,
            parameter_type_identities,
            result_type_identity,
            validated_physical_plan.contract_report_fingerprint(),
            physical_realization.boundary_entry_plan.clone(),
        )
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic)])?;
        let storage_entry = storage_entry
            .with_physical_contract(physical_contract)
            .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
        Ok(SelectedProgramEntryCallingPlans {
            semantic_calling_application: semantic_realization
                .native_application()
                .map_err(|reason| vec![Diagnostic::error(reason)])?,
            physical_calling_application: physical_realization
                .native_application()
                .map_err(|reason| vec![Diagnostic::error(reason)])?,
            storage_entry,
        })
    }
}

mod extent_layout {
    use crate::provider_planning::type_reference_node;
    use calling_conventions::{BoundaryValueClass, BoundaryValueField, ValueShape};
    use checked_trees::typed_trees::TypedTrees;
    use checked_trees::typed_trees::types::{
        PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
    };
    use layout::BoundaryValueLayout;

    pub(super) fn capture(
        typed: &TypedTrees,
        slot: target::ProgramEntrySlotDeclaration,
        type_reference: TypeReferenceHandle,
    ) -> Result<program_entry_plan::ProgramEntryExtentValueLayout, String> {
        if slot.owner != target::TargetProfile::UefiX64
            || slot.schema != target::ProgramEntrySchema::ProgramStorageApplication
            || slot.visible_parameters
                != target::ProgramEntryVisibleParameters::ImageAndInitialStorage
            || slot.semantic_calling_convention
                != Some(target::ProgramEntryCallingConvention::MicrosoftX64)
        {
            return Err(
                "selected-source Extent layout derivation is restricted to the exact UEFI/Microsoft program-storage schema"
                    .into(),
            );
        }
        let mut preparation =
            checked_trees::typed_trees::boundary_values::BoundaryValuePreparation::new(
                typed,
                &[],
                &[],
            );
        let recipe = preparation.value_type(type_reference);
        let recipes = preparation.finish();
        let mut values = BoundaryValueLayout::new(layout::LayoutTarget::for_profile(slot.owner)?);
        let (shape, root) = values.value_type(&recipes, recipe)?;
        let (shapes, fields, _) = values.into_parts();
        let mut base_type = type_reference;
        while let TypeReferenceNode::Constrained {
            base_type: unconstrained,
            ..
        } = type_reference_node(typed, base_type)
        {
            base_type = *unconstrained;
        }
        let Some(data_symbol) = type_reference_node(typed, base_type).as_named_symbol() else {
            return Err("selected program-storage root is not a named Extent record".into());
        };
        let definition = typed
            .data_definition_by_symbol(data_symbol)
            .ok_or_else(|| "selected program-storage Extent has no data definition".to_owned())?;
        let [
            checked_trees::typed_trees::data::DataMember::Field(base),
            checked_trees::typed_trees::data::DataMember::Field(length),
        ] = typed.data_members(definition)
        else {
            return Err("selected program-storage Extent must declare exactly two fields".into());
        };
        if !data_symbol.is_valid()
            || !base.symbol.is_valid()
            || !length.symbol.is_valid()
            || base.symbol == length.symbol
            || base.relevance.is_erased()
            || length.relevance.is_erased()
            || base.name.as_str() != "base"
            || length.name.as_str() != "length"
            || typed.primitive_type_reference(base.type_reference) != Some(PrimitiveType::Addr)
            || typed.primitive_type_reference(length.type_reference) != Some(PrimitiveType::U64)
        {
            return Err(
                "selected program-storage Extent must declare exact `base: addr; length: u64` fields"
                    .into(),
            );
        }
        let root = shapes.get(usize::from(root)).ok_or_else(|| {
            "selected program-storage Extent lost its record shape root".to_owned()
        })?;
        let BoundaryValueClass::Record {
            first_field,
            field_count: 2,
        } = root.class
        else {
            return Err(
                "selected program-storage Extent is not an exact two-field record graph".into(),
            );
        };
        let normalized_fields = fields
            .get(usize::from(first_field)..usize::from(first_field) + 2)
            .ok_or_else(|| {
                "selected program-storage Extent field graph is out of bounds".to_owned()
            })?;
        let [base_field, length_field] = normalized_fields else {
            return Err("selected program-storage Extent field graph is out of bounds".into());
        };
        let scalar_shape = |field: &BoundaryValueField| -> Result<ValueShape, String> {
            let child = shapes.get(usize::from(field.shape)).ok_or_else(|| {
                "selected program-storage Extent child shape is out of bounds".to_owned()
            })?;
            if !matches!(child.class, BoundaryValueClass::Integer) {
                return Err(
                    "selected program-storage Extent field is not an integer scalar".into(),
                );
            }
            Ok(ValueShape::integer(
                u32::from(child.byte_size),
                child.alignment,
            ))
        };
        program_entry_plan::ProgramEntryExtentValueLayout::from_checked_record(
            base_field.byte_offset,
            scalar_shape(base_field)?,
            length_field.byte_offset,
            scalar_shape(length_field)?,
            shape,
        )
    }
}
