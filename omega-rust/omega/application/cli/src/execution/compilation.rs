//! `run` is a native product consumer, not a focused-file compiler shortcut.
//! Package preparation and native acceptance stay in the manager, as for the
//! ordinary compile command. Reference execution consumes the exact produced
//! Terminal artifact and its retained runtime provisions before publication
//! transfers that payload. Rechecking source or interpreting checked trees would
//! bypass the product contract and could repeat Build or lose generated inputs.

use compiler::runtime::{TerminalRuntimeError, TerminalRuntimeObservation, TerminalRuntimeRequest};
use compiler::{CompileOptions, CompileReport, CompileRequest};
use diagnostics::Diagnostic;
use package_manager::operations::LocalProjectPreparationOptions;
use package_manager::operations::{
    CompilePreparedLocalProjectNativeError, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};

// This is an explicit sponsor budget, not a divergence verdict. Exhaustion
// remains an unavailable comparison and may be raised for a measured workload.
const REFERENCE_EXECUTION_FUEL_ALLOWANCE: u64 = 10_000_000;

pub(super) struct ProbeCompilation {
    pub report: CompileReport,
    pub interpretation: Option<Result<TerminalRuntimeObservation, TerminalRuntimeError>>,
}

pub(super) fn compile(
    mut options: CompileOptions,
    compare_interpreter: bool,
) -> Result<ProbeCompilation, Vec<Diagnostic>> {
    let build_dir = options.retain_build_dir();
    let target = crate::invocation_target_profile(options.target_name.as_deref())
        .map_err(|diagnostic| vec![diagnostic])?;
    let prepared = prepare_local_project(
        &options.root_path,
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )
    .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let report = if let Some(prepared) = prepared {
        let request = PreparedLocalProjectNativeRequest::new(prepared, build_dir, target);
        compile_prepared_local_project_for_native(request, |_| ())
            .map(|(report, ())| report)
            .map_err(|error| match error {
                // Realization diagnostics keep their exceeded provisions, so a
                // compile that stopped at a private provision reads as
                // `Incomplete` rather than as one flattened rejection message.
                CompilePreparedLocalProjectNativeError::Native(diagnostics) => diagnostics,
                error => vec![Diagnostic::error(error.to_string())],
            })?
    } else {
        compiler::compile(
            CompileRequest::new(options)
                .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)?
    };
    let interpretation = compare_interpreter.then(|| interpret_product(&report));
    Ok(ProbeCompilation {
        report,
        interpretation,
    })
}

fn interpret_product(
    report: &CompileReport,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let artifact = report
        .retained_native_artifact()
        .map(|artifact| artifact.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|artifact| artifact.psi_artifact())
        })
        .ok_or(TerminalRuntimeError::InvalidProvision(
            "native report lost its produced Terminal artifact before comparison",
        ))?;
    compiler::runtime::execute(TerminalRuntimeRequest {
        artifact,
        provisions: report.runtime_provisions(),
        fuel_allowance: REFERENCE_EXECUTION_FUEL_ALLOWANCE,
    })
}
