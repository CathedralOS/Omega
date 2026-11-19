//! Admission identities needed after executable source graphs have been dropped.

use target_vocabulary::publication::{DeploymentEntryFacts, NativeEntryFacts};

/// Closed entry, installation and execution-scope facts. This record contains
/// neither an artifact nor a source lookup capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionAdmission {
    entry: NativeEntryFacts,
    native_callbacks: provider_admission::AdmittedNativeCallbackCohort,
    installed_interrupts: provider_admission::AdmittedInstalledInterruptCohort,
}

impl super::AdmittedLoweringFacts {
    pub fn emission_admission(&self) -> EmissionAdmission {
        let module = self.module();
        let entry = module.machine(self.entry);
        let deployment = self.target_context.as_ref().map(|selection| DeploymentEntryFacts {
            terminal: selection.terminal_psi(),
            profile: target::TargetProfile::from_canonical_target_name(selection.target())
                .map_err(|_| Box::from(selection.target())),
            machine: selection
                .entry(self.terminal, module, terminal_psi::TargetMachineCallable::Machine(self.entry))
                .map_err(|_| "native object entry has no executable member at its admitted deployment"),
        });
        EmissionAdmission {
            entry: NativeEntryFacts {
                deployment,
                entry_has_body: entry.is_some(),
                terminal: self.terminal,
                entry: self.entry,
                original_entry: module.entry,
                identities: module.identities.clone(),
                entry_declaration: entry.and_then(|entry| entry.declaration.clone()),
                native_execution_scope: self
                    .native_execution_scope
                    .as_ref()
                    .map(|scope| std::sync::Arc::clone(scope.reach.emitted())),
                selected_provider_closure: self.selected_provider_closure,
            },
            native_callbacks: self.native_callbacks.clone(),
            installed_interrupts: self.installed_interrupts.clone(),
        }
    }
}

impl EmissionAdmission {
    /// An explicit audit retains the exact lowering that issued these records.
    /// Equal reached IDs from independent provider selections have distinct owners.
    pub(super) fn belongs_to(&self, lowering: &super::AdmittedLoweringFacts) -> bool {
        let same_scope = match (
            &self.entry.native_execution_scope,
            &lowering.native_execution_scope,
        ) {
            (Some(carried), Some(issued)) => std::sync::Arc::ptr_eq(carried, issued.reach.emitted()),
            (None, None) => true,
            _ => false,
        };
        same_scope && self == &lowering.emission_admission()
    }

    pub fn entry_facts(&self) -> &NativeEntryFacts {
        &self.entry
    }

    pub fn native_callbacks(&self) -> &provider_admission::AdmittedNativeCallbackCohort {
        &self.native_callbacks
    }
    pub fn installed_interrupts(&self) -> &provider_admission::AdmittedInstalledInterruptCohort {
        &self.installed_interrupts
    }
}

/// The final body and the selected physical adapter must be the same terminal
/// machine. Keep just its ordinary entry, semantic entry and inbound formals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedPhysicalEntry(program_entry_contracts::PhysicalEntrySelection);
impl AsRef<program_entry_contracts::PhysicalEntrySelection> for AdmittedPhysicalEntry {
    fn as_ref(&self) -> &program_entry_contracts::PhysicalEntrySelection {
        &self.0
    }
}
impl EmissionAdmission {
    pub fn physical_entry(
        &self,
        entry: &crate::ValidatedNativeProgramEntrySettlement,
    ) -> Result<AdmittedPhysicalEntry, &'static str> {
        let adapter = entry
            .physical_adapter()
            .ok_or("physical entry has no selected adapter")?;
        let checked = entry.checked_entry();
        if entry.source().target_slot().owner != target::TargetProfile::UefiX64
            || adapter.terminal() != self.entry.terminal
            || checked.terminal_psi_identity() != self.entry.terminal
            || adapter.machine() != self.entry.entry
        {
            return Err("physical entry changed its selected UEFI contract");
        }
        Ok(AdmittedPhysicalEntry(
            program_entry_contracts::PhysicalEntrySelection {
                psi: self.entry.terminal,
                adapter: adapter.machine(),
                semantic_entry: checked.terminal_entry(),
                inbound_ordinals: adapter.parameter_formals().to_vec(),
            },
        ))
    }
}
