//! Execute source-authored project requests through the public Terminal SDK.

use build_declarations::{BuildDependencySource, BuildRequest, DependencyPurpose};
use build_evaluation::terminal_requests::TerminalBuildRequestHost;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram, TerminalStructuralInputs,
    TerminalStructuralValue,
};

pub(super) fn observe(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    requests: &psi_compiler::TerminalBuildRequests,
) -> Result<(), String> {
    let loaded = TerminalProgram::load_envelope(&artifact.to_bytes())
        .map_err(|error| format!("load: {error:?}"))?;
    for program in [TerminalProgram::borrow(artifact.view()), loaded] {
        let entry = program
            .entry()
            .map_err(|error| format!("entry: {error:?}"))?;
        let [parameter] = entry.structural_parameters else {
            return Err("project request entry needs its original Build receiver".into());
        };
        let original = TerminalStructuralValue::minted(955, parameter.structural_type, Vec::new());
        for foreign in [false, true] {
            let host = TerminalBuildRequestHost::new(
                artifact.view().into(),
                requests,
                original.clone(),
                None,
            )?;
            let mut receiver = original.clone();
            if foreign {
                receiver.opaque_identity += 1;
            }
            let mut instance = program
                .instantiate(terminal_interpreter::TerminalInstantiation {
                    structural_inputs: TerminalStructuralInputs {
                        arguments: std::slice::from_ref(&receiver),
                        zero_initialized_arguments: &[0],
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .map_err(|error| format!("instantiate: {error:?}"))?;
            for boundary in instance.unbound_imports().collect::<Vec<_>>() {
                let declaration = artifact
                    .semantic_module()
                    .boundary_machines
                    .iter()
                    .find(|declaration| declaration.id == boundary)
                    .ok_or("missing import declaration")?;
                instance
                    .bind_import(boundary, host.adapter(declaration.into()))
                    .map_err(|error| format!("bind: {error:?}"))?;
            }
            if foreign {
                let error = instance
                    .invoke(&mut TerminalFuelMeter::unbounded())
                    .err()
                    .ok_or("foreign Build receiver accepted")?;
                if !format!("{error:?}").contains("original compiler-issued Build") {
                    return Err(format!(
                        "foreign receiver refused for another reason: {error:?}"
                    ));
                }
                if !host.drain_requests()?.is_empty() {
                    return Err("foreign receiver issued a request".into());
                }
                continue;
            }
            let mut observed = Vec::new();
            let mut fuel = TerminalFuelMeter::with_allowance(0);
            for _ in 0..2_000 {
                let status = instance
                    .invoke(&mut fuel)
                    .map_err(|error| format!("invoke: {error:?}"))?;
                observed.extend(host.drain_requests()?);
                match status {
                    TerminalExecutionStatus::SponsorExhausted(_) => fuel
                        .replenish(1)
                        .map_err(|error| format!("fuel: {error:?}"))?,
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => break,
                    other => return Err(format!("unexpected Build status: {other:?}")),
                }
            }
            if instance.result().is_none() {
                return Err("project requests did not complete".into());
            }
            let [role, dependency] = observed.as_slice() else {
                return Err(format!(
                    "expected two executed project requests, observed {}",
                    observed.len()
                ));
            };
            if !matches!(role.request(), BuildRequest::Application(name) if name.as_str() == "computed_project")
                || !matches!(dependency.request(), BuildRequest::Dependency { purpose: DependencyPurpose::Product, alias: Some(alias), source: BuildDependencySource::Path(location) }
                    if alias.as_str() == "selected" && location == "selected_source")
            {
                return Err(
                    "executed Build role/dependency payload differs from its oracle".into(),
                );
            }
            instance
                .invoke(&mut fuel)
                .map_err(|error| format!("cached completion: {error:?}"))?;
            if !host.drain_requests()?.is_empty() {
                return Err("cached completion replayed project effects".into());
            }
            instance
                .close()
                .map_err(|error| format!("close: {error:?}"))?;
        }
    }
    Ok(())
}
