//! Production-report subject projection from one complete checked artifact.

use super::CheckedCompilation;
use diagnostics::Diagnostic;

impl CheckedCompilation {
    /// The production-report subject of a package-aware compilation, or
    /// `None` for a compilation without package custody. Every product stop
    /// consumes this projection rather than reconstructing its parts.
    pub fn production_subject(
        &self,
    ) -> Result<Option<crate::ProductionCompilationSubject>, Vec<Diagnostic>> {
        let Some(package) = self.sources().package_subject() else {
            return Ok(None);
        };
        let build_machine = self
            .build_admission()
            .selected_build_machine_identity
            .as_deref()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "package production requires one exact selected build-machine identity",
                )]
            })?;
        let usage = self.build().evaluation_usage.ok_or_else(|| {
            vec![Diagnostic::error(
                "package production requires exact build-evaluation accounting",
            )]
        })?;
        let observation = self.build().observation_summary.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "package production requires exact build-observation custody",
            )]
        })?;
        let profile = self.execution().selected_target_profile.ok_or_else(|| {
            vec![Diagnostic::error(
                "package production requires one selected target profile",
            )]
        })?;
        let native = self.execution().selected_native_target().ok_or_else(|| {
            vec![Diagnostic::error(
                "package production requires one selected native target",
            )]
        })?;
        crate::ProductionCompilationSubject::from_checked(
            package.clone(),
            build_machine.to_owned(),
            usage,
            observation,
            profile,
            native,
        )
        .map(Some)
        .map_err(|message| vec![Diagnostic::error(message)])
    }
}
