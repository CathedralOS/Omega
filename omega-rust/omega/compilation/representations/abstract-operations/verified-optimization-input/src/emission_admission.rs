//! Admission identities needed after executable source graphs have been dropped.

use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_psi::{TerminalIdentityIndex, TerminalPsiIdentity};

/// Closed entry, installation and execution-scope facts. This record contains
/// neither an artifact nor a source lookup capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionAdmission {
    terminal: TerminalPsiIdentity,
    entry: MachineId,
    original_entry: Option<terminal_psi::TargetMachineCallable>,
    identities: terminal_psi::TerminalIdentityTable,
    entry_declaration: Option<terminal_psi::CheckedMachineDeclaration>,
    entry_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    carriers: Vec<(StructuralTypeId, TerminalIdentityIndex)>,
    native_execution_scope: Option<std::sync::Arc<super::NativeExecutionScope>>,
    selected_provider_closure: Option<[u8; 32]>,
    target_context: Option<terminal_installation::TerminalTargetContext>,
    native_callbacks: provider_admission::AdmittedNativeCallbackCohort,
    installed_interrupts: provider_admission::AdmittedInstalledInterruptCohort,
}

impl super::AdmittedLoweringFacts {
    pub fn emission_admission(&self) -> EmissionAdmission {
        let module = self.module();
        let entry = module.machine(self.entry);
        EmissionAdmission {
            terminal: self.terminal,
            entry: self.entry,
            original_entry: module.entry,
            identities: module.identities.clone(),
            entry_declaration: entry.and_then(|entry| entry.declaration.clone()),
            entry_parameters: entry.map_or_else(Vec::new, |entry| entry.structural_parameters.clone()),
            carriers: module.structural_types.iter().map(|carrier| (carrier.id, carrier.identity)).collect(),
            native_execution_scope: self.native_execution_scope.clone(),
            selected_provider_closure: self.selected_provider_closure,
            target_context: self.target_context.clone(),
            native_callbacks: self.native_callbacks.clone(),
            installed_interrupts: self.installed_interrupts.clone(),
        }
    }
}

impl EmissionAdmission {
    /// Check current entry custody without reopening an earlier executable graph.
    pub fn selected_terminal_entry(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        entry: MachineId,
        native_target: target::NativeTarget,
    ) -> Result<Option<MachineId>, &'static str> {
        let identity = artifact.view().identity();
        if self.terminal != identity || self.entry != entry {
            return Err("native object entry differs from its admitted executable input");
        }
        let Some(selections) = self.target_context() else { return Ok(None); };
        let profile = target::TargetProfile::from_canonical_target_name(selections.target())
            .map_err(|_| "native object entry has an unknown admitted deployment tag")?;
        if profile.native_realization() != Some(native_target) {
            return Err("native object target differs from its admitted deployment profile");
        }
        selections.entry(identity, artifact.semantic_module(), terminal_psi::TargetMachineCallable::Machine(entry))
            .map(Some).map_err(|_| "native object entry has no executable member at its admitted deployment")
    }

    pub const fn terminal(&self) -> TerminalPsiIdentity { self.terminal }
    pub const fn entry(&self) -> MachineId { self.entry }
    pub const fn original_entry(&self) -> Option<terminal_psi::TargetMachineCallable> { self.original_entry }
    pub fn identities(&self) -> &terminal_psi::TerminalIdentityTable { &self.identities }
    pub fn entry_declaration(&self) -> Option<&terminal_psi::CheckedMachineDeclaration> { self.entry_declaration.as_ref() }
    pub fn target_context(&self) -> Option<&terminal_installation::TerminalTargetContext> { self.target_context.as_ref() }
    pub fn native_callbacks(&self) -> &provider_admission::AdmittedNativeCallbackCohort { &self.native_callbacks }
    pub fn installed_interrupts(&self) -> &provider_admission::AdmittedInstalledInterruptCohort { &self.installed_interrupts }

    pub fn access_view_input_parameter(&self, row: &terminal_psi::TerminalAccessViewInput) -> Option<&terminal_psi::StructuralParameterDeclaration> {
        let declaration = self.entry_declaration.as_ref()?;
        if row.machine != self.entry
            || declaration.machine.identity() != row.source_machine_identity
            || declaration.state.identity() != row.source_state_identity
            || !self.carriers.iter().any(|(_, identity)| *identity == row.carrier_identity)
        { return None; }
        let parameter = self.entry_parameters.get(row.runtime_position as usize)?;
        let (_, identity) = self.carriers.iter().find(|(id, _)| *id == parameter.structural_type)?;
        (*identity == row.carrier_identity
            && parameter.access == row.access
            && row.access != terminal_psi::StructuralAccess::Owned).then_some(parameter)
    }

    pub fn native_execution_scope_for(
        &self,
        terminal: TerminalPsiIdentity,
        target: target::NativeTarget,
        entry: MachineId,
        selected_provider_closure: [u8; 32],
    ) -> Result<Option<&super::NativeExecutionScope>, &'static str> {
        let Some(scope) = self.native_execution_scope.as_deref() else { return Ok(None); };
        let selections = self.target_context.as_ref().ok_or("native execution scope has no target selection custody")?;
        let profile = target::TargetProfile::from_canonical_target_name(selections.target())
            .map_err(|_| "native execution scope names an unknown target")?;
        if self.terminal != terminal
            || selections.terminal_psi() != terminal
            || profile.native_target() != target
            || self.entry != entry
            || self.selected_provider_closure != Some(selected_provider_closure)
        {
            return Err("native execution scope belongs to another artifact, target, entry or provider selection");
        }
        Ok(Some(scope))
    }
}

/// The final body and the selected physical adapter must be the same terminal
/// machine. Keep just its ordinary entry, semantic entry and inbound formals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedPhysicalEntry(program_entry_plan::physical_selection::PhysicalEntrySelection);
impl AsRef<program_entry_plan::physical_selection::PhysicalEntrySelection>
    for AdmittedPhysicalEntry
{
    fn as_ref(&self) -> &program_entry_plan::physical_selection::PhysicalEntrySelection {
        &self.0
    }
}
impl EmissionAdmission {
    pub fn physical_entry(
        &self,
        entry: &crate::ValidatedNativeProgramEntrySettlement,
    ) -> Result<AdmittedPhysicalEntry, &'static str> {
        let adapter = entry.physical_adapter().ok_or("physical entry has no selected adapter")?;
        let checked = entry.checked_entry();
        if entry.source().target_slot().owner != target::TargetProfile::UefiX64
            || adapter.terminal() != self.terminal()
            || checked.terminal_psi_identity() != self.terminal()
            || adapter.machine() != self.entry()
        {
            return Err("physical entry changed its selected UEFI contract");
        }
        Ok(AdmittedPhysicalEntry(
            program_entry_plan::physical_selection::PhysicalEntrySelection {
                psi: self.terminal(),
                adapter: adapter.machine(),
                semantic_entry: checked.terminal_entry(),
                inbound_ordinals: adapter.parameter_formals().to_vec(),
            },
        ))
    }
}
