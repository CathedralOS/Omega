// Program facts read by provider admission, and installed private-entry
// execution verification.
//
// Terminal-to-abstract lowering (Omega 00) hands admission these facts from its
// verified artifact input and its admitted component calls. Admission reads
// them as data; the sealed evidence it mints stays private to this package.
//
// An installed private entry runs at CPL0 only after the parent's CPU check and
// publication. That condition belongs to the private object rather than every
// occurrence of the same source machine, so final composition retains and
// checks the same installing Scope and entry through
// `AdmittedInstalledEntryExecution` before exposing the body.

use std::sync::Arc;

use abstract_operations::AbstractOperationPlan;
use calling_conventions::BoundaryCallingApplication;
use component_description::VerifiedComponent;
use effects::provider_plan::ProviderPlanDigest;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_installation::{AdmittedComponentProvider, AdmittedTargetMachineSelections};
use terminal_psi::TerminalModule;

use crate::{
    AdmittedInstalledInterruptCohort, AdmittedInstalledInterruptEntry,
    AdmittedInstalledInterruptScope,
};

#[derive(Debug, Clone, Copy)]
pub struct AdmissionSource<'a> {
    pub plan: &'a AbstractOperationPlan,
    pub module: &'a TerminalModule,
    pub target_machine_selections: Option<&'a AdmittedTargetMachineSelections>,
    pub program_entry: Option<AdmittedProgramEntryCall>,
    pub private_entry_ram: &'a [terminal_installation::CheckedPrivateEntryRamRecipe],
}

impl<'a> AdmissionSource<'a> {
    pub const fn plan(&self) -> &'a AbstractOperationPlan {
        self.plan
    }

    pub const fn module(&self) -> &'a TerminalModule {
        self.module
    }

    pub const fn target_machine_selections(&self) -> Option<&'a AdmittedTargetMachineSelections> {
        self.target_machine_selections
    }

    pub const fn selected_program_entry_call(&self) -> Option<&AdmittedProgramEntryCall> {
        self.program_entry.as_ref()
    }

    pub const fn private_entry_ram(
        &self,
    ) -> &'a [terminal_installation::CheckedPrivateEntryRamRecipe] {
        self.private_entry_ram
    }

    pub fn target_family_member(
        &self,
        boundary: BoundaryMachineId,
    ) -> Result<Option<terminal_psi::TargetMachineCallable>, String> {
        terminal_installation::admitted_boundary_member(
            self.module,
            self.target_machine_selections,
            boundary,
        )
    }

    pub fn selected_calling_abi_identity(
        &self,
        boundary: BoundaryMachineId,
    ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
        let declaration =
            selected_calling_boundary(self.module, self.target_machine_selections, boundary)?;
        self.module.calling_abi_identity(declaration)
    }
}

pub fn selected_calling_boundary(
    module: &terminal_psi::TerminalModule,
    targets: Option<&terminal_installation::AdmittedTargetMachineSelections>,
    boundary: semantic_vocabulary::BoundaryMachineId,
) -> Result<semantic_vocabulary::BoundaryMachineId, String> {
    match terminal_installation::admitted_boundary_member(module, targets, boundary)? {
        Some(terminal_psi::TargetMachineCallable::Boundary(member)) => Ok(member),
        Some(terminal_psi::TargetMachineCallable::Machine(_)) => {
            Err("checked target-family member has no external calling declaration".into())
        }
        None => Ok(boundary),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedProgramEntryCall {
    pub boundary: BoundaryMachineId,
    pub entry: MachineId,
    pub provider_plan: ProviderPlanDigest,
}

impl AdmittedProgramEntryCall {
    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn provider_plan(&self) -> ProviderPlanDigest {
        self.provider_plan
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ComponentCallFacts<'a> {
    pub selection: &'a AdmittedComponentProvider,
    pub component: &'a VerifiedComponent,
    pub application: &'a BoundaryCallingApplication,
    pub plan_digest: &'a ProviderPlanDigest,
}

impl<'a> ComponentCallFacts<'a> {
    pub const fn selection(&self) -> &'a AdmittedComponentProvider {
        self.selection
    }

    pub const fn component(&self) -> &'a VerifiedComponent {
        self.component
    }

    pub const fn application(&self) -> &'a BoundaryCallingApplication {
        self.application
    }

    pub const fn plan_digest(&self) -> &'a ProviderPlanDigest {
        self.plan_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledEntryExecution {
    scope: Arc<AdmittedInstalledInterruptScope>,
    entry: AdmittedInstalledInterruptEntry,
}

impl AdmittedInstalledEntryExecution {
    pub fn for_entry(
        scope: Arc<AdmittedInstalledInterruptScope>,
        entry: &AdmittedInstalledInterruptEntry,
        current: &AdmittedInstalledInterruptCohort,
    ) -> Result<Self, &'static str> {
        Self::check_current(&scope, entry, current)?;
        Ok(Self {
            scope,
            entry: entry.clone(),
        })
    }

    pub fn require_current(
        &self,
        current: &AdmittedInstalledInterruptCohort,
    ) -> Result<(), &'static str> {
        Self::check_current(&self.scope, &self.entry, current)
    }

    fn check_current(
        scope: &AdmittedInstalledInterruptScope,
        entry: &AdmittedInstalledInterruptEntry,
        current: &AdmittedInstalledInterruptCohort,
    ) -> Result<(), &'static str> {
        if current.psi() != entry.entry().psi()
            || entry.operation != scope.operation()
            || entry.entry().target().architecture != target::Architecture::X86_64
            || (!scope.fatal_entries().is_empty()
                && scope.interrupt_delivery_assumption().is_none())
            || scope.bootstrap_assumption().is_none_or(|assumption| {
                assumption.psi != current.psi() || assumption.installation != scope.operation()
            })
        {
            return Err("installed execution changed its program, target or parent interval");
        }
        let mut entries = current
            .entries()
            .iter()
            .filter(|candidate| candidate.placement_index == entry.placement_index);
        if entries.next() != Some(entry) || entries.next().is_some() {
            return Err("installed execution has no exact unique current entry");
        }
        let mut scopes = current
            .scopes()
            .iter()
            .filter(|candidate| candidate.operation() == scope.operation());
        let Some(received) = scopes.next() else {
            return Err("installed execution lost its current installation");
        };
        if scopes.next().is_some()
            || received.service() != scope.service()
            || received.boundary() != scope.boundary()
            || received.arguments() != scope.arguments()
            || received.table_type() != scope.table_type()
            || received.context_type() != scope.context_type()
            || received.validator() != scope.validator()
            || received.work() != scope.work()
            || received.completion() != scope.completion()
            || received.acknowledgement_application() != scope.acknowledgement_application()
            || received
                .fatal_entries()
                .iter()
                .any(|entry| !scope.fatal_entries().contains(entry))
            || received
                .context_anchor()
                .is_some_and(|anchor| Some(anchor) != scope.context_anchor())
            || (!received.installation_assumptions().is_empty()
                && received.installation_assumptions() != scope.installation_assumptions())
        {
            return Err("installed execution substituted its parent installation");
        }
        Ok(())
    }

    pub fn scope(&self) -> &AdmittedInstalledInterruptScope {
        &self.scope
    }

    pub fn entry(&self) -> &AdmittedInstalledInterruptEntry {
        &self.entry
    }
}
