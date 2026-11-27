//! Project the surviving installations of the prepared program before selecting guards.
use bound_installations::BoundInstalledEntry;
use installation_conditions::InstalledInterruptStackRequirement;
use legalized_operations::PreparedExecutionPlan;
use provider_admission::{
    AdmittedInstalledEntryExecution, AdmittedInstalledInterruptCohort, AdmittedNativeCallbackCohort,
};

use installation_admission::PlannedInstallationResources;

/// The authorized handlers and measured stacks available to one target body.
pub struct InstallationRequirements<'a> {
    pub interrupts: &'a AdmittedInstalledInterruptCohort,
    pub callbacks: &'a AdmittedNativeCallbackCohort,
    pub entry_execution: Option<AdmittedInstalledEntryExecution>,
    pub stack: Vec<InstalledInterruptStackRequirement>,
}

impl InstallationRequirements<'_> {
    pub fn bind(
        self,
        prepared: &PreparedExecutionPlan,
    ) -> Result<PlannedInstallationResources, &'static str> {
        let plan = prepared.plan();
        installation_admission::admit_planned_guards(
            BoundInstalledEntry {
                psi: plan.psi,
                target: plan.target,
                machine: plan.entry,
            },
            self.interrupts,
            self.callbacks,
            self.entry_execution,
            self.stack,
            &plan.installation_operations,
        )
    }
}
