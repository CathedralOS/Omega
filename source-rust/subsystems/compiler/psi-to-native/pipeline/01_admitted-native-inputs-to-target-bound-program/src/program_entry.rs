// Select the entry named by Build and retain its actual checked declaration.
// Arrival and provider plans are supplied separately from the same program.

use diagnostics::Diagnostic;
mod calling_plans;
mod published;
pub use calling_plans::{ProgramEntryCallingPlanRequest, entry_calling_policy};
pub use provider_acceptance::program_entry_semantic_binding_role;
pub use program_entry_contracts::ProgramEntryPhysicalContractPackageSourceDigest;
pub use published::{PublishedEntryNomination, plan_published_entry_calling_conventions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramEntryCallingPlans {
    pub semantic_calling_application: calling_conventions::BoundaryCallingApplication,
    pub physical_calling_application: calling_conventions::BoundaryCallingApplication,
    pub storage_entry: program_entry_contracts::SelectedProgramStorageEntryPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCompilerProgramEntry {
    signature: program_entry_contracts::SelectedProgramEntrySignature,
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
        let entry_block = body
            .blocks
            .iter()
            .find(|block| block.id == body.entry)
            .ok_or_else(|| refuse("the selected entry has no entry block".into()))?;
        if !body.parameters.is_empty()
            || !entry_block.parameters.is_empty()
            || !entry_block.structural_parameters.is_empty()
            || !entry_block.erased_scalar_formals.is_empty()
            || !entry_block.erased_proof_formals.is_empty()
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
        if module.identities.text(checked.callable).is_none_or(str::is_empty) {
            return Err(refuse("selected entry lost its checked callable identity".into()));
        }
        let signature =
            program_entry_contracts::SelectedProgramEntrySignature::from_checked_declaration(
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
        declarations: &[provider_plans::ProviderPlan],
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
            let plan = declaration;
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
                    provider_plans::ProviderBinding::CheckedAdapter { .. }
                ) {
                    return Err(vec![Diagnostic::error(
                        "selected physical entry adapter is absent",
                    )]);
                }
                let header = target_provider_admission::selected_checked_adapter_header(plan, row, module)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                if !header.supplies_checked_body || header.type_parameter_count != 0 {
                    return Err(vec![Diagnostic::error(
                        "physical entry adapter must have a checked source body",
                    )]);
                }
                let rows = target_provider_admission::selected_provider_candidate_rows(plan, module)
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

    pub const fn source_signature(
        &self,
    ) -> &program_entry_contracts::SelectedProgramEntrySignature {
        &self.signature
    }

    pub const fn calling_plans(&self) -> Option<&SelectedProgramEntryCallingPlans> {
        self.calling_plans.as_ref()
    }
}
