//! Execute the admitted Build through Terminal, then join its returned requests.
mod project;

use crate::{AdmittedBuildProgram, BuildConfig, ComputedBuildConfig};
use diagnostics::Diagnostic;
use evaluation_accounting::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits};

pub(crate) fn execute_admitted_build_program(
    admitted: AdmittedBuildProgram,
) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
    let AdmittedBuildProgram {
        prepared,
        machine,
        filesystem_scope,
        evaluation_sponsor,
        artifact_only: _,
    } = admitted;
    let refuse = |reason: String| {
        vec![Diagnostic::error(format!(
            "Terminal Build `{}` failed: {reason}",
            machine.name
        ))]
    };
    // Unconnected filesystem adapters cannot execute the legacy prelude's
    // placeholder bodies and then claim that the activation observed no I/O.
    if machine.filesystem_reachable {
        return Err(refuse(
            "Terminal Build filesystem operations are not installed".into(),
        ));
    }
    let mut observations = package_compilation::CapturedBuildObservations::new(&filesystem_scope);
    let checked = prepared
        .checked_trees()
        .ok_or_else(|| refuse("Build has no admitted source judgment".into()))?;
    let toolchain =
        source::source_identity::toolchain_source_identities(checked.symbols.source_files())
            .map_err(&refuse)?;
    let mut production = psi_compiler::TerminalProductionRequest::new(
        checked,
        psi_compiler::TerminalMachineSelection::Symbol(machine.symbol),
        &toolchain,
    );
    production.publication = terminal_codec::TerminalPublication::Invocation;
    let produced = production
        .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
        .map_err(|error| error.diagnostics())?;

    // These are Terminal units. An existing tree-measured sponsor is never
    // relabeled or silently replaced by a fresh account.
    let measurement = crate::terminal_execution::measurement();
    const INVOCATION_FUEL: u64 = 100_000;
    let sponsor = match evaluation_sponsor {
        Some(sponsor) => sponsor,
        None => {
            // Standalone compilation previously had only a per-invocation work
            // ceiling. Its private account adds measurement, not filesystem grants.
            let limits = BuildEvaluationSponsorLimits::new(
                INVOCATION_FUEL,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
            )
            .ok_or_else(|| refuse("invalid standalone Build limits".into()))?;
            BuildEvaluationSponsor::new_with_measurement(limits, measurement)
                .map_err(|error| refuse(error.to_string()))?
        }
    };
    let account = sponsor
        .begin_invocation(measurement, INVOCATION_FUEL)
        .map_err(|error| refuse(error.to_string()))?;
    let mut requests = Vec::new();
    artifacts::compile_timings::counters::record_build_execution();
    let completed = crate::terminal_execution::execute(
        &produced,
        machine.value_admission,
        &machine.initial_build,
        &account,
        |issued| {
            requests.extend(issued);
            Ok(())
        },
    )
    .map_err(&refuse)?;
    let declaration = project::declaration(&requests).map_err(&refuse)?;
    // No Product may consume an outstanding edge. This refusal remains until
    // the application coordinator installs the real request/join protocol.
    if requests.iter().any(|issued| {
        matches!(
            issued.request(),
            build_declarations::BuildRequest::Dependency { .. }
        )
    }) {
        return Err(refuse(
            "dependency requests require coordinator settlement before Product compilation".into(),
        ));
    }
    if matches!(&declaration, build_declarations::BuildDeclaration::Application(application) if application.artifact_only)
    {
        return Err(refuse(
            "artifact-only Build requires completed output custody".into(),
        ));
    }
    observations.retain_build_log(completed.log.publish(&account).map_err(&refuse)?)?;
    let observation_summary = observations.settle(machine.filesystem_reachable, &machine.name)?;
    let config = BuildConfig {
        settings: completed.settings,
        ..Default::default()
    };
    let usage = *account
        .snapshot()
        .map_err(|error| refuse(error.to_string()))?
        .record();
    Ok(ComputedBuildConfig {
        config,
        root_binding_requests: completed.root_bindings,
        project_declaration: Some(declaration),
        optimization_report_request: completed.optimization_report,
        evaluation_usage: Some(usage),
        observation_summary: Some(observation_summary),
        selected_build_machine_symbol: Some(machine.symbol),
        generated_sources: Vec::new(),
    })
}
