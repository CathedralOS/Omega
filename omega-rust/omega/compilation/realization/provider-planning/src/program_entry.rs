// Typed root-binding metadata and retained native entry data survive the old
// checker. Source-contract preparation and checked service-reach settlement
// have no producer here; canonical entry/adapter readers remain exact.

use diagnostics::Diagnostic;
pub use language_semantics::declaration_selection::RootBinding;
pub use package_compilation::program_entry_semantic_binding_role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedProgramEntry<'config> {
    pub machine_name: &'config str,
    pub machine_symbol: symbols::SymbolHandle, // Exact machine selected under the binding occurrence's lexical package.
    pub slot: target::ProgramEntrySlotDeclaration,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedEntryDeclaration {
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    signature: program_entry_plan::SelectedProgramEntrySignature,
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
