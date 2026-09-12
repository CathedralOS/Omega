//! `run` is a native product consumer, not a focused-file compiler shortcut.
//! Package preparation and native acceptance stay in the manager, as for the
//! ordinary compile command. The optional interpreter observes that same checked
//! package root, including generated inputs; rechecking the authored file would
//! lose those inputs and could repeat build effects or acquire different sources.

use checked_interpreter::InterpretOutcome;
use compiler::{ArtifactEmissionPolicy, CompileOptions, CompileReport, CompileRequest};
use diagnostics::Diagnostic;
use package_manager::operations::{
    PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native_with_observation,
    prepare_local_project_for_target,
};

pub(super) struct ProbeCompilation {
    pub report: CompileReport,
    pub interpretation: Option<Result<InterpretOutcome, Vec<Diagnostic>>>,
}

pub(super) fn compile(
    mut options: CompileOptions,
    artifact_policy: ArtifactEmissionPolicy,
    interpret: bool,
) -> Result<ProbeCompilation, Vec<Diagnostic>> {
    let build_dir = options.retain_build_dir();
    let target = target::TargetProfile::from_omega_target_name(options.target_name.as_deref())
        .map_err(|diagnostic| vec![diagnostic])?;
    let prepared = prepare_local_project_for_target(&options.root_path, target)
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    // Policy belongs to the authored project, never its resolver snapshot.
    let admissions = trust_ledger::read_trust_admissions(&options.root_path)?;
    let (report, interpretation) = if let Some(prepared) = prepared {
        let request = PreparedLocalProjectNativeRequest::new(prepared, build_dir, target)
            .with_artifact_policy(artifact_policy)
            .with_accepted_trust_admissions(admissions);
        compile_prepared_local_project_for_native_with_observation(request, |checked| {
            interpret.then(|| interpret_checked(checked))
        })
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?
    } else {
        let report = compiler::compile(
            CompileRequest::new(options.clone())
                .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact)
                .with_artifact_policy(artifact_policy)
                .with_accepted_trust_admissions(admissions),
        )?;
        let interpretation = interpret.then(|| {
            compiler::compile_to_checked(&options.root_path, Some(target.target_name()))
                .and_then(|checked| interpret_checked(&checked))
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
) -> Result<InterpretOutcome, Vec<Diagnostic>> {
    let entry = checked.selected_program_entry_machine().ok_or_else(|| {
        vec![Diagnostic::error(
            "build has no exact target-owned ProgramEntry binding",
        )]
    })?;
    // Default interpretation captures output and uses virtual host state. Do not
    // print or grant live host effects before native admission and publication.
    Ok(checked_interpreter::interpret_entry(checked, entry, &[]))
}
