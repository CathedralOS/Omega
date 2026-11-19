pub fn inspect_terminal(
    request: &InspectTerminalRequest,
) -> Result<TerminalInspection, InspectTerminalError> {
    let targets = match request.target_name.as_deref() {
        Some(name) => BuildTargets::Single(
            target::TargetProfile::from_omega_target_name(Some(name))
                .map_err(|error| InspectTerminalError::Diagnostics(vec![error]))?,
        ),
        None => BuildTargets::Host,
    };
    let outcomes = orchestration::build(BuildRequest {
        stage_validators: Default::default(),
        root_path: request.root_path.clone(),
        build_directory: None,
        targets,
        product: ProjectProduct::Inspection {
            machine: request.machine.clone(),
        },
        timings: false,
        offline: false,
        verify: true,
        require_package_project: false,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .map_err(InspectTerminalError::Build)?;
    let [outcome]: [_; 1] = outcomes.try_into().map_err(|_| {
        InspectTerminalError::Diagnostics(vec![Diagnostic::error(
            "inspection requires exactly one project Build outcome",
        )])
    })?;
    let outcome = outcome.result.map_err(InspectTerminalError::Build)?;
    let artifact = outcome
        .output
        .into_inspection()
        .map_err(|error| InspectTerminalError::Diagnostics(error.into_diagnostics()))?;
    Ok(TerminalInspection {
        artifact: std::sync::Arc::clone(artifact.artifact()),
    })
}
