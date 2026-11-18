//! Project the surviving installations of the prepared program before selecting guards.
use bound_installations::BoundInstalledEntry;
use installation_evidence::InstalledInterruptStackRequirement;
use legalized_operations::PreparedExecutionPlan;
use provider_admission::{
    AdmittedInstalledEntryExecution, AdmittedInstalledInterruptCohort, AdmittedNativeCallbackCohort,
};

pub use installation_admission::PlannedInstallationResources;

pub fn plan_installation_resources(
    prepared: &PreparedExecutionPlan,
    cohort: &AdmittedInstalledInterruptCohort,
    callbacks: &AdmittedNativeCallbackCohort,
    entry_execution: Option<AdmittedInstalledEntryExecution>,
    requirements: Vec<InstalledInterruptStackRequirement>,
) -> Result<PlannedInstallationResources, &'static str> {
    let plan = prepared.plan();
    installation_admission::admit_planned_guards(
        BoundInstalledEntry {
            psi: plan.psi,
            target: plan.target,
            machine: plan.entry,
        },
        cohort,
        callbacks,
        entry_execution,
        requirements,
        &plan.installation_operations,
    )
}
