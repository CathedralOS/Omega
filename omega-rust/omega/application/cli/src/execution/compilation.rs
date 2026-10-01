//! `run` is a native product consumer, not a focused-file compiler shortcut.
//! Package preparation and native acceptance stay in the manager, as for the
//! ordinary compile command. The optional interpreter observes that same checked
//! package root, including generated inputs; rechecking the authored file would
//! lose those inputs and could repeat build effects or acquire different sources.

use checked_interpreter::{BuildMachineEntry, InterpretOptions, InterpretOutcome};
use compiler::CheckedCompileRequest;
use compiler::{CompileOptions, CompileReport, CompileRequest};
use diagnostics::Diagnostic;
use package_manager::operations::LocalProjectPreparationOptions;
use package_manager::operations::{
    CompilePreparedLocalProjectNativeError, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};

pub(super) struct ProbeCompilation {
    pub report: CompileReport,
    pub interpretation: Option<Result<InterpretOutcome, Vec<Diagnostic>>>,
}

pub(super) fn compile(
    mut options: CompileOptions,
    interpreter_input: Option<&[u8]>,
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
    // Policy belongs to the authored project, never its resolver snapshot.
    let admissions = trust_ledger::read_trust_admissions(&options.root_path)?;
    let (report, interpretation) = if let Some(prepared) = prepared {
        let request = PreparedLocalProjectNativeRequest::new(prepared, build_dir, target)
            .with_accepted_trust_admissions(admissions);
        compile_prepared_local_project_for_native(request, |checked| {
            interpreter_input.map(|input| interpret_checked(checked, input))
        })
        .map_err(|error| match error {
            // Realization diagnostics keep their exceeded provisions, so a
            // compile that stopped at a private provision reads as
            // `Incomplete` rather than as one flattened rejection message.
            CompilePreparedLocalProjectNativeError::Native(diagnostics) => diagnostics,
            error => vec![Diagnostic::error(error.to_string())],
        })?
    } else {
        let report = compiler::compile(
            CompileRequest::new(options.clone())
                .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact)
                .with_accepted_trust_admissions(admissions),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)?;
        let interpretation = interpreter_input.map(|input| {
            compiler::compile_to_checked(CheckedCompileRequest::new(
                &options.root_path,
                Some(target.target_name()),
            ))
            .and_then(|checked| interpret_checked(&checked, input))
        });
        (report, interpretation)
    };
    Ok(ProbeCompilation {
        report,
        interpretation,
    })
}

fn interpret_checked(
    checked: &compiler::CheckedCompilation,
    input: &[u8],
) -> Result<InterpretOutcome, Vec<Diagnostic>> {
    let entry = checked.selected_program_entry().ok_or_else(|| {
        vec![Diagnostic::error(
            "build has no exact target-owned ProgramEntry binding",
        )]
    })?;
    // Dispatch on the bound machine's symbol, never its spelling: another
    // module or package may declare a same-named machine.
    // Default interpretation captures output and uses virtual host state. Do not
    // print or grant live host effects before native admission and publication.
    // The entry shape, not the interpreter, names the context occurrence the
    // arrival bridge provisions.
    let signature = entry.source_signature();
    let mut options = InterpretOptions::default();
    if let Some(context) = signature.receiver().source_symbol() {
        options = options.with_provisioned_context_parameter(context);
    }
    Ok(checked_interpreter::interpret_entry(
        checked,
        BuildMachineEntry::Symbol(signature.machine_symbol()),
        input,
        options,
    ))
}
