//! Optional writing of already-admitted checked-program observations.

use crate::{ArtifactEmissionPolicy, CheckedAdmission, CompileOptions};
use artifacts::ArtifactWriter;
use diagnostics::Diagnostic;

impl CheckedAdmission<'_> {
    /// Write requested observations without changing admission or the program.
    /// Output-only requests perform no observation filesystem operations.
    pub fn write_observations(
        &self,
        options: &CompileOptions,
        policy: ArtifactEmissionPolicy,
    ) -> Result<(), Vec<Diagnostic>> {
        if policy.emits_auxiliary_artifacts() {
            let writer =
                ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
            writer
                .write_trust_report(self.trust_report())
                .map_err(|diagnostic| vec![diagnostic])?;
            let checked = self.checked();
            crate::pipeline::artifacts::write_checked_snapshots(
                &writer,
                checked,
                checked.selected_program_entry_machine(),
                checked.selected_provider_plans(),
                checked.task_activations(),
                checked.component_progress(),
            )?;
            writer
                .write_timings(checked.timings().phases())
                .map_err(|diagnostic| vec![diagnostic])?;
        }
        Ok(())
    }
}
