//! Production-report subject projection from one complete checked artifact.

use psi_diagnostics::Diagnostic;

pub(crate) fn project_production_subject(
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<Option<omega_compilation_report::ProductionCompilationSubject>, Vec<Diagnostic>> {
    let Some(package) = checked.package_compilation_subject() else {
        return Ok(None);
    };
    let build_machine = checked.selected_build_machine_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "package production requires one exact selected build-machine identity",
        )]
    })?;
    let usage = checked.build_evaluation_usage().ok_or_else(|| {
        vec![Diagnostic::error(
            "package production requires exact build-evaluation accounting",
        )]
    })?;
    let observation = checked.build_observation_summary().ok_or_else(|| {
        vec![Diagnostic::error(
            "package production requires exact build-observation custody",
        )]
    })?;
    let profile = checked.selected_target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "package production requires one selected target profile",
        )]
    })?;
    let native = checked.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "package production requires one selected native target",
        )]
    })?;
    omega_compilation_report::ProductionCompilationSubject::from_checked(
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
