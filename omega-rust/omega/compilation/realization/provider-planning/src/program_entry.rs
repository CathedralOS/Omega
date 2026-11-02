//! Program-entry selection and validation of source and physical calling contracts.
//!
//! Entry slots select a contract owner before selecting declarations. Bundled
//! contracts use their exact source custody; accepted packages use their explicit
//! package/declaration binding. The physical requirement comes from that schema's
//! resolved composition graph; the semantic requirement retains core's identity.
//! Calling plans must join both exact requirement symbols to the selected schema.
//! Unrelated same-leaf traits cannot redirect selection, including when Build and
//! product retain separate package instances.

use crate::provider_planning::entry_state;
pub use build_time_evaluation::product_entries::RootBinding;
use build_time_evaluation::product_entries::{
    validate_source_compatibility, visible_entry_parameters,
};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::product_operand::ProductEntryDeclaration;
use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use diagnostics::Diagnostic;

mod calling_plans;
mod discovery;
mod extent_layout;
mod source_contract;
pub use discovery::discover_explicit_entry_binding;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedProgramEntry<'config> {
    pub machine_name: &'config str,
    /// Exact machine selected under the binding occurrence's lexical package.
    pub machine_symbol: symbols::SymbolHandle,
    pub slot: target::ProgramEntrySlotDeclaration,
}

/// Resolve the selected target's `ProgramEntry` binding. This is the first
/// implemented target-root slot; other root-slot kinds reject rather than
/// being accepted and then ignored. A build file may describe a target matrix:
/// well-formed slots owned by other known profiles are validated and left for
/// those profiles, while this selection consumes only the chosen profile's
/// exact row. With no root declarations there is no selected entry; native
/// production independently requires one before realization.
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

/// Recheck the selected source signature and retain its final layout evidence.
/// An earlier product description neither settles providers nor authorizes
/// omission of this check against the completed product.
fn validate_selected_program_entry_shape(
    typed: &TypedTrees,
    selected: SelectedProgramEntry<'_>,
) -> Result<SelectedEntryDeclaration, Vec<Diagnostic>> {
    let machine_name = selected.machine_name;
    // Description issuance only needs an interface. Final admission still
    // requires the exact selected executable machine and its real entry state.
    let machine = typed.machine(selected.machine_symbol)
        .ok_or_else(|| vec![Diagnostic::error(format!(
            "build root slot selected entry `{machine_name}` is not a declaration in the admitted program"
        ))])?;
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
                extent_layout::capture(
                    typed,
                    selected.slot,
                    parameter.type_reference,
                )
            .map_err(|diagnostic| {
                Diagnostic::error(format!(
                    "selected entry machine `{machine_name}` visible parameter {index} has no exact Extent value layout: {diagnostic}"
                ))
            })?;
            let value_shape = extent_value_layout.shape();
            Ok(program_entry_plan::SelectedProgramEntrySignature::visible_parameter(
                role,
                index,
                identities
                    .intern(&typed.normalized_type_identity_text(parameter.type_reference)),
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
    // Only this declaration's owner participates in its nominal identity. Do
    // not hash the entire source set to close one entry's source coordinate.
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

/// Source-only coordinates stay beside the producer that can interpret them.
/// Native consumers receive `signature`, never this record.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedEntryDeclaration {
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    signature: program_entry_plan::SelectedProgramEntrySignature,
}

/// The source judgment for an exact target entry, before any calling policy
/// runs. Retaining it does not imply that physical arrival has been settled.
/// The source producer can issue this judgment independently of ABI placement;
/// completing it below consumes the subsequently evaluated calling plans.
pub struct PreparedCompilerProgramEntry {
    declaration: SelectedEntryDeclaration,
    slot: target::ProgramEntrySlotDeclaration,
    calling_contract:
        Result<Option<source_contract::PreparedEntryCallingContract>, Vec<Diagnostic>>,
}

impl PreparedCompilerProgramEntry {
    /// Save the services this entry can call before target binding drops source.
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

    /// Join target Calling results to the exact Source contract prepared from
    /// this invocation's immutable package bindings. No Source is reopened.
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

/// Retain both authored applications: their source/signature commitments are
/// distinct from the validated ABI plans' own structural commitments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramEntryCallingPlans {
    pub semantic_calling_application: calling_conventions::BoundaryCallingApplication,
    pub physical_calling_application: calling_conventions::BoundaryCallingApplication,
    pub storage_entry: program_entry_plan::SelectedProgramStorageEntryPlan,
}

/// Complete realization-owned settlement for one target-selected `ProgramEntry`.
///
/// The source signature and optional two-surface calling plans are validated
/// while typed declarations are still available, then travel together instead
/// of becoming independent driver couriers. A test-harness entry-name override
/// is deliberately absent: it cannot acquire source, calling-plan, or storage
/// authority through this carrier.
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

    /// The explicitly selected checked implementation of physical arrival.
    /// This is separate from the semantic ProgramEntry: firmware cannot call
    /// the two-root continuation with its handle and System Table registers.
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
                // The shared program owns the adapter's diagnostic name/span.
                // Absence here preserves that attribution instead of rebuilding it.
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

/// Shared checking for bound rows the request does not realize. One Build
/// produces checked rows for every target it names, so an authored binding
/// carries the same source obligations whether or not its target is selected:
/// the bound machine must exist, own an executable entry state, and satisfy
/// its slot's source compatibility. What stays per target is absence, not
/// malformation — a family with no body for an unselected target and an
/// unbound required slot wait for that target's own selection.
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
        // Mirror the selected path's family resolution: the binding names the
        // family, and only the body authored for its own target answers the
        // source-shape check.
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

/// Issue the exact selected entry's source contract without evaluating or
/// consuming a calling policy. Slot selection and source-shape refusals retain
/// their order; physical/semantic calling validation belongs to settlement.
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
    // A root binding names the family, not the body that happens to own its
    // canonical spelling. Select the exact entry handle without changing the
    // source graph; every body still undergoes the ordinary Psi checks.
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
    // Capture remains inert until after provider selection. Source formation,
    // target Calling conventions and physical provenance keep their original
    // refusal order even though no late Source borrow remains.
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

pub use package_compilation::program_entry_semantic_binding_role;
