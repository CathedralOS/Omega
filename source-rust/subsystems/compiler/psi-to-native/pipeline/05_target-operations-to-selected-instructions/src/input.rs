//! The prepared target body and the installations bound to its entry.
use legalized_operations::{LegalizedOperationPlan, PreparationReceipt, PreparedExecutionPlan};

pub struct SelectionInput {
    pub body: target_lowering::NativeBodyPlan,
    pub entry_execution: Option<provider_admission::AdmittedInstalledEntryExecution>,
    pub stack: Vec<installation_conditions::InstalledInterruptStackRequirement>,
}

/// Installation authority instruction selection projects from the admitted body after
/// preparation: the interrupt installations, the installed entry, and
/// the stack each installed handler needs. It grants permission and
/// deployment resources; the plan alone says what the program does.
pub use bound_installations::BoundInstallations;

/// Why a set of installations cannot be used with a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationBindingError {
    InterruptCohortProgram,
    EntryExecution,
    StackRequirement,
}

/// A plan together with installations that were checked to belong to it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoundExecutionPlan<'input> {
    prepared: &'input PreparedExecutionPlan,
    installations: &'input BoundInstallations,
    pub(crate) remainder_mask: bool,
    pub(crate) scaled_loads: bool,
}

impl<'input> BoundExecutionPlan<'input> {
    /// Installations name the program, target and entry they were
    /// admitted for. Refuse any that name another plan.
    pub(crate) fn bind(
        prepared: &'input PreparedExecutionPlan,
        installations: &'input BoundInstallations,
    ) -> Result<Self, InstallationBindingError> {
        let plan = prepared.plan();
        if installations.psi != plan.psi {
            return Err(InstallationBindingError::InterruptCohortProgram);
        }
        if installations.installed_entry.is_some_and(|entry| {
            entry.psi != plan.psi || entry.target != plan.target || entry.machine != plan.entry
        }) {
            return Err(InstallationBindingError::EntryExecution);
        }
        if installations
            .stack_requirements
            .iter()
            .any(|requirement| requirement.psi != plan.psi || requirement.target != plan.target)
        {
            return Err(InstallationBindingError::StackRequirement);
        }
        Ok(Self {
            prepared,
            installations,
            remainder_mask: false,
            scaled_loads: false,
        })
    }

    pub(crate) fn plan(&self) -> &'input LegalizedOperationPlan {
        self.prepared.plan()
    }

    pub(crate) fn callable_invocations(
        &self,
    ) -> &'input terminal_semantics::callable_values::ReachableCallableInvocations {
        self.prepared.callable_invocations()
    }

    pub(crate) fn receipt(&self) -> PreparationReceipt {
        self.prepared.receipt()
    }

    pub(crate) fn installations(&self) -> &'input BoundInstallations {
        self.installations
    }
}
