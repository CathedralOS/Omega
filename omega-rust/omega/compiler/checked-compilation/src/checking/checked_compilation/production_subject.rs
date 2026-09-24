//! Production-report subject projection from one complete checked artifact.

use super::CheckedCompilation;
use diagnostics::Diagnostic;

impl CheckedCompilation {
    /// The production-report subject of a package-aware compilation, or
    /// `None` for a compilation without package custody. Every product stop
    /// consumes this projection rather than reconstructing its parts.
    pub fn production_subject(
        &self,
    ) -> Result<Option<compilation_report::ProductionCompilationSubject>, Vec<Diagnostic>> {
        let Some(package) = self.package_compilation_subject() else {
            return Ok(None);
        };
        let build_machine = self.selected_build_machine_identity().ok_or_else(|| {
            vec![Diagnostic::error(
                "package production requires one exact selected build-machine identity",
            )]
        })?;
        let usage = self.build_evaluation_usage().ok_or_else(|| {
            vec![Diagnostic::error(
                "package production requires exact build-evaluation accounting",
            )]
        })?;
        let observation = self.build_observation_summary().ok_or_else(|| {
            vec![Diagnostic::error(
                "package production requires exact build-observation custody",
            )]
        })?;
        let profile = self.selected_target_profile().ok_or_else(|| {
            vec![Diagnostic::error(
                "package production requires one selected target profile",
            )]
        })?;
        let native = self.selected_native_target().ok_or_else(|| {
            vec![Diagnostic::error(
                "package production requires one selected native target",
            )]
        })?;
        compilation_report::ProductionCompilationSubject::from_checked(
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
