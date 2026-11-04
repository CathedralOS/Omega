//! What provider admission reads from the program it admits providers for.
//!
//! Terminal-to-abstract lowering (Omega 00) hands admission these facts from
//! its verified artifact input and its admitted component calls. Admission
//! reads them as data; the sealed evidence it mints stays private to this
//! package.

use abstract_operations::AbstractOperationPlan;
use calling_conventions::BoundaryCallingApplication;
use component_description::VerifiedComponent;
use effects::provider_plan::ProviderPlanDigest;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_installation::{AdmittedComponentProvider, AdmittedTargetMachineSelections};
use terminal_psi::TerminalModule;

/// The admitted program: its abstract plan, its Terminal module, the
/// deployment's target machine selections and the selected program entry.
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

    /// Resolve a portable family head to its exact execution coordinate under
    /// the deployment's selections.
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

    /// The calling ABI of the declaration a boundary's selected member uses.
    pub fn selected_calling_abi_identity(
        &self,
        boundary: BoundaryMachineId,
    ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
        let declaration =
            selected_calling_boundary(self.module, self.target_machine_selections, boundary)?;
        self.module.calling_abi_identity(declaration)
    }
}

/// Family membership is admitted independently of native planning. Proposal
/// construction and later native admission use this same declaration join.
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

/// The program entry call the admitted provider plan selected.
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

/// One admitted component call: the selected component provider, the
/// verified component, the calling application and the provider plan.
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
