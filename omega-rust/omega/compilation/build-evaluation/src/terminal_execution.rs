//! Run one source-issued Build artifact, retaining its original receiver.
//!
//! This is the execution owner, not source discovery or package resolution.
//! The coordinator receives executed requests between fuel slices and must join
//! them before publishing a product. Both protocol adapters borrow one activation;
//! a pause never reconstructs the instance or replays earlier effects.

use build_time_evaluation::BuildTimeValue;
use evaluation_accounting::EvaluationInvocationAccount;
use language_semantics::declaration_selection::ExecutedRootBinding;
use package_compilation::build_execution::build_value::{
    BuildValueAdmission, EvaluatedBuildSettings,
};
use psi_compiler::ProducedTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation, TerminalProgram,
    TerminalStructuralInputs, TerminalStructuralValue,
};

use crate::terminal_bindings::TerminalBuildBindingHost;
use crate::terminal_requests::{IssuedBuildRequest, TerminalBuildRequestHost};

/// Normal-return values only. Dependency requests remain with the coordinator,
/// whose mandatory join precedes normalized Build publication.
pub(crate) struct TerminalBuildCompletion {
    pub settings: EvaluatedBuildSettings,
    pub optimization_report: optimization_core::OptimizationReportRequest,
    pub root_bindings: Vec<ExecutedRootBinding>,
}

/// Execute an admitted toolchain Build over its zero-initialized original
/// backing. The caller supplies the same account to invocation and host effects;
/// arbitrary runtime imports never receive a Build adapter.
pub(crate) fn execute(
    produced: &ProducedTerminalArtifact,
    admission: BuildValueAdmission,
    initial: &BuildTimeValue,
    account: &EvaluationInvocationAccount,
    mut submit: impl FnMut(Vec<IssuedBuildRequest>) -> Result<(), String>,
) -> Result<TerminalBuildCompletion, String> {
    let artifact = produced.artifact();
    let program = TerminalProgram::borrow(artifact.view());
    let entry = program.entry().map_err(describe)?;
    let [parameter] = entry.structural_parameters else {
        return Err("Build activation requires its one original structural receiver".into());
    };
    if !entry.scalar_parameters.is_empty() {
        return Err("Build activation has unexpected scalar parameters".into());
    }
    // Identity is invocation-local: neither a second instance nor a same-typed
    // guest constructor inherits this host's right to submit requests.
    let original = TerminalStructuralValue::minted(1, parameter.structural_type, Vec::new());
    let requests = TerminalBuildRequestHost::new(
        artifact,
        produced.build_requests(),
        original.clone(),
        Some(account),
    )?;
    let bindings =
        TerminalBuildBindingHost::new(artifact, produced.build_bindings(), original.clone())?;
    let mut instance = program
        .instantiate(TerminalInstantiation {
            structural_inputs: TerminalStructuralInputs {
                arguments: std::slice::from_ref(&original),
                zero_initialized_arguments: &[0],
                ..Default::default()
            },
            account: Some(account),
            ..Default::default()
        })
        .map_err(describe)?;

    let request_calls = produced
        .build_requests()
        .calls_for(artifact)
        .ok_or("Build requests lost their artifact custody")?;
    let binding_calls = produced
        .build_bindings()
        .calls_for(artifact)
        .ok_or("Build bindings lost their artifact custody")?;
    for boundary in instance.unbound_imports().collect::<Vec<_>>() {
        let declaration = artifact
            .semantic_module()
            .boundary_machines
            .iter()
            .find(|declaration| declaration.id == boundary)
            .ok_or("Build import has no Terminal declaration")?;
        if request_calls.iter().any(|call| call.boundary == boundary) {
            instance
                .bind_import(boundary, requests.adapter(declaration.into()))
                .map_err(describe)?;
        } else if binding_calls.iter().any(|call| call.boundary == boundary) {
            instance
                .bind_import(boundary, bindings.adapter(declaration.into()))
                .map_err(describe)?;
        } else {
            return Err(format!(
                "Build import {boundary:?} has no admitted host operation or installed provider"
            ));
        }
    }

    // A slice is scheduling, not a fresh allowance: the shared account imposes
    // the total admitted ceiling across every resume and emitted request.
    const SLICE_FUEL: u64 = 1024;
    let mut fuel =
        TerminalFuelMeter::with_invocation(account.clone(), Some(SLICE_FUEL)).map_err(describe)?;
    loop {
        let status = instance.invoke(&mut fuel).map_err(describe)?;
        let issued = requests.drain_requests()?;
        if !issued.is_empty() {
            submit(issued)?;
        }
        match status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => break,
            TerminalExecutionStatus::SponsorExhausted(_) => {
                fuel.replenish(SLICE_FUEL).map_err(describe)?;
            }
            other => return Err(format!("Build did not return normally: {other:?}")),
        }
    }
    let (settings, optimization_report) =
        crate::terminal_settings::extract(&instance, admission, initial)?;
    instance.close().map_err(describe)?;
    drop(instance);
    Ok(TerminalBuildCompletion {
        settings,
        optimization_report,
        root_bindings: bindings.into_executed()?,
    })
}

fn describe(error: impl std::fmt::Debug) -> String {
    format!("Terminal Build: {error:?}")
}
