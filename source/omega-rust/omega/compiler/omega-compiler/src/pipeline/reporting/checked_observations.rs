//! Checked-compilation validation and optional observation emission.
//!
//! Trust reconstruction, admission settlement, and trust-report consistency
//! checks are semantic validations and therefore run for every request. The
//! observation policy controls only the single writer branch below.

use crate::compiler::{ArtifactEmissionPolicy, CompileOptions};
use crate::pipeline::CheckedCompilation;
use omega_artifacts::ArtifactWriter;
use psi_diagnostics::Diagnostic;

/// One complete checked observation request. Keeping the checked surface and
/// owner-supplied trust policy together prevents the product driver from
/// couriering individual report inputs or branching on observation policy.
pub(crate) struct CheckedObservationInput<'a> {
    pub(crate) options: &'a CompileOptions,
    pub(crate) artifact_policy: ArtifactEmissionPolicy,
    pub(crate) accepted_trust_admissions: &'a [omega_trust_model::TrustAdmission],
    pub(crate) checked: &'a CheckedCompilation,
}

/// Validate all checked trust evidence and optionally emit its observations.
pub(crate) fn report_checked_observations(
    input: CheckedObservationInput<'_>,
) -> Result<omega_trust_model::TrustAdmissionSettlement, Vec<Diagnostic>> {
    let obligations = omega_trust_model::reconstruct_trust_obligations(
        &input.checked.typed,
        input.checked,
        input.checked.root_grants(),
        input.checked.provider_plans(),
        input.checked.selected_provider_plans(),
        input.checked.accepted_template_classifications(),
        input.checked.package_identity().is_some(),
    )?;
    let settlement =
        omega_trust_model::settle_trust_admissions(obligations, input.accepted_trust_admissions)
            .map_err(|diagnostic| vec![diagnostic])?;
    let trust_report = omega_trust_model::reconstruct_trust_report(
        input.checked,
        input.checked.root_grants(),
        input.checked.provider_plans(),
        input.checked.selected_provider_plans(),
        input.checked.accepted_template_classifications(),
    )?;
    trust_report
        .validate()
        .map_err(|diagnostic| vec![diagnostic])?;

    if input.artifact_policy.emits_auxiliary_artifacts() {
        let writer = ArtifactWriter::new(&input.options.build_dir())
            .map_err(|diagnostic| vec![diagnostic])?;
        writer
            .write_trust_report(&trust_report)
            .map_err(|diagnostic| vec![diagnostic])?;
        crate::pipeline::artifacts::write_checked_snapshots(
            &writer,
            input.checked,
            input.checked.selected_program_entry_machine(),
            input.checked.selected_provider_plans(),
            input.checked.task_activations(),
            input.checked.component_progress(),
        )?;
        writer
            .write_timings(input.checked.timings().phases())
            .map_err(|diagnostic| vec![diagnostic])?;
    }

    Ok(settlement)
}
