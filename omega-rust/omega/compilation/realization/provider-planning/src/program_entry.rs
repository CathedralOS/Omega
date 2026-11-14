// Select the entry named by Build and retain its actual checked declaration.
// Arrival and provider plans are supplied separately from the same program.

use diagnostics::Diagnostic;
mod calling_plans;
pub use calling_plans::ProgramEntryCallingPlanRequest;
pub use language_semantics::declaration_selection::RootBinding;
pub use package_compilation::program_entry_semantic_binding_role;
pub use program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedProgramEntry<'config> {
    pub binding_ordinal: usize,
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
    for (ordinal, binding) in root_bindings.iter().enumerate() {
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
        selected_bindings.push((required_slot, binding, ordinal));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut program_entries = Vec::new();
    for required_slot in selected_profile.required_root_slots() {
        let matches = selected_bindings
            .iter()
            .filter(|(selected, _, _)| selected == &required_slot)
            .collect::<Vec<_>>();
        let (binding, ordinal) = match matches.as_slice() {
            [(_, binding, ordinal)] => (*binding, *ordinal),
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
        program_entries.push((program_entry, binding, ordinal));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    match program_entries.as_slice() {
        [(slot, binding, ordinal)] => Ok(Some(SelectedProgramEntry {
            binding_ordinal: *ordinal,
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
pub struct SelectedProgramEntryCallingPlans {
    pub semantic_calling_application: calling_conventions::BoundaryCallingApplication,
    pub physical_calling_application: calling_conventions::BoundaryCallingApplication,
    pub storage_entry: program_entry_plan::SelectedProgramStorageEntryPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCompilerProgramEntry {
    signature: program_entry_plan::SelectedProgramEntrySignature,
    calling_plans: Option<SelectedProgramEntryCallingPlans>,
    physical_adapter: Option<terminal_psi::PreparedNominalRoot>,
}

impl SelectedCompilerProgramEntry {
    pub fn from_terminal(
        module: &terminal_psi::TerminalModule,
        machine: semantic_vocabulary::MachineId,
        slot: target::ProgramEntrySlotDeclaration,
    ) -> Result<Self, Vec<Diagnostic>> {
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let body = module.machine(machine).ok_or_else(|| {
            refuse("the selected entry has no independently published machine".into())
        })?;
        let checked = body
            .declaration
            .as_ref()
            .ok_or_else(|| refuse("the selected entry has no checked declaration".into()))?;
        if body.result != terminal_psi::TerminalMachineResult::Unit {
            return Err(refuse("the selected program entry must return Unit".into()));
        }
        // The installed contract, rather than a receiver's shape, determines
        // which source arguments the environment must establish.
        if slot.visible_parameters != target::ProgramEntryVisibleParameters::None {
            return Err(refuse(format!(
                "no product route yet: entry `{}` needs its installed visible storage arguments derived from the checked arrival contract",
                module
                    .identities
                    .text(checked.machine.identity())
                    .unwrap_or("<absent>"),
            )));
        }
        if !body.parameters.is_empty()
            || !body.structural_parameters.is_empty()
            || !body.parameter_order.is_empty()
            || !checked.parameter_types.is_empty()
        {
            return Err(refuse(
                "the selected hosted entry exposes arguments its slot does not provide".into(),
            ));
        }
        let declaration = module
            .identities
            .text(checked.machine.identity())
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                refuse("the selected entry lost its nominal declaration identity".into())
            })?;
        let signature =
            program_entry_plan::SelectedProgramEntrySignature::from_checked_declaration(
                slot,
                declaration.to_owned(),
                declaration.to_owned(),
                module
                    .identities
                    .text(checked.state.identity())
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| refuse("selected entry lost its checked state identity".into()))?
                    .to_owned(),
                checked.callable,
                Vec::new(),
                module.identities.clone(),
            )
            .map_err(refuse)?;
        Ok(Self {
            signature,
            calling_plans: None,
            physical_adapter: None,
        })
    }

    pub fn nominal_root(&self) -> Result<terminal_psi::PreparedNominalRoot, Vec<Diagnostic>> {
        terminal_psi::PreparedNominalRoot::new(
            self.signature.declaration(),
            self.machine_name(),
            None,
        )
        .map_err(|reason| vec![Diagnostic::error(reason.to_string())])
    }

    pub fn with_calling_plans(
        mut self,
        plans: SelectedProgramEntryCallingPlans,
    ) -> Result<Self, Vec<Diagnostic>> {
        let slot = self.signature.target_slot();
        let refuse = |reason: &str| vec![Diagnostic::error(reason)];
        if plans.storage_entry.target_slot() != slot
            || plans.semantic_calling_application.target != slot.owner.native_target()
            || plans.physical_calling_application.target != slot.owner.native_target()
        {
            return Err(refuse(
                "entry arrival plans belong to another selected target",
            ));
        }
        let semantic = plans
            .storage_entry
            .identities()
            .text(plans.storage_entry.requirement_identity())
            .ok_or_else(|| refuse("entry storage plan lost its semantic arrival identity"))?;
        let physical = plans
            .storage_entry
            .physical_contract()
            .ok_or_else(|| refuse("entry storage plan has no checked physical arrival contract"))?;
        if plans.semantic_calling_application.requirement_identity != semantic
            || physical.identities().text(physical.requirement_identity())
                != Some(
                    plans
                        .physical_calling_application
                        .requirement_identity
                        .as_str(),
                )
        {
            return Err(refuse(
                "entry arrival plans name different requirements from their storage contract",
            ));
        }
        for application in [
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
        ] {
            if application.commitment == [0; 32]
                || application.report_fingerprint == 0
                || application
                    .recomputed_commitment()
                    .map_err(|reason| vec![Diagnostic::error(reason)])?
                    != application.commitment
            {
                return Err(refuse(
                    "entry arrival plan lost its checked calling application",
                ));
            }
        }
        self.calling_plans = Some(plans);
        Ok(self)
    }

    pub fn machine_name(&self) -> &str {
        self.signature.machine_name()
    }

    pub fn physical_adapter(&self) -> Option<&terminal_psi::PreparedNominalRoot> {
        self.physical_adapter.as_ref()
    }

    pub fn bind_physical_adapter(
        &mut self,
        module: &terminal_psi::TerminalModule,
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
        for declaration in declarations {
            let plan = &declaration.plan;
            for row in &plan.rows {
                if plan.identity_text(row.requirement_identity) != requirement {
                    continue;
                }
                let owner = declaration.provider_declaration.as_deref().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "physical entry adapter lost its selecting owner",
                    )]
                })?;
                let mut choices = module.provider_selections.iter().filter(|choice| {
                    choice.target == plans.storage_entry.target_slot().owner.target_name()
                        && module.identities.text(choice.provider_declaration) == Some(owner)
                        && module
                            .service_schema(choice.schema_declaration, choice.schema_application)
                            .is_ok_and(|schema| {
                                schema.surface.as_ref() == plan.schema.declaration.as_ref()
                            })
                });
                if choices.next().is_none() || choices.next().is_some() {
                    return Err(vec![Diagnostic::error(
                        "physical entry adapter has no unique explicit source provider choice",
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
                let header = backend_plan::selected_checked_adapter_header(plan, row, module)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                if !header.supplies_checked_body || header.type_parameter_count != 0 {
                    return Err(vec![Diagnostic::error(
                        "physical entry adapter must have a checked source body",
                    )]);
                }
                let rows = backend_plan::selected_provider_candidate_rows(plan, owner, module)
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
        &self.signature
    }

    pub const fn calling_plans(&self) -> Option<&SelectedProgramEntryCallingPlans> {
        self.calling_plans.as_ref()
    }
}
