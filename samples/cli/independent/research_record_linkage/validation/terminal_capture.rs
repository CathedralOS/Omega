//! Private observation consumer of the ordinary admitted native product.
//! No package decisions, source interpreter or alternate runtime route.
use compiler::runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::{error::Error, fs, path::PathBuf};
use terminal_interpreter::TerminalExecutionStatus;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: terminal_capture ORIGINAL_SAMPLE_ROOT OUTPUT_DIRECTORY".into());
    }
    let root = PathBuf::from(&args[0]).canonicalize()?;
    let output = PathBuf::from(&args[1]);
    fs::create_dir_all(&output)?;
    let target = target::TargetProfile::host();
    let prepared = prepare_local_project(
        &root.join("main.omg"),
        LocalProjectPreparationOptions { target, offline: true },
    ).map_err(|error| error.to_string())?.ok_or("missing prepared project")?;
    let report = compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, output.join("native-build"), target),
    ).map_err(|error| error.to_string())?;
    let native = report.retained_native_artifact().ok_or("missing native artifact")?;
    let artifact = native.psi_artifact();
    fs::write(output.join("application.terminal"), artifact.to_bytes())?;
    fs::write(output.join("compiler.identity"), compiler::compiler_identity())?;
    let mut fixtures: Vec<_> = fs::read_dir(root.join("data"))?
        .map(|entry| entry.map(|entry| entry.path())).collect::<Result<_, _>>()?;
    fixtures.retain(|path| path.extension().is_some_and(|suffix| suffix == "txt"));
    fixtures.sort();
    if fixtures.len() != 39 { return Err("expected 39 frozen input fixtures".into()); }
    let mut records = String::from("case\texit\n");
    for input in fixtures {
        let name = input.file_stem().and_then(|name| name.to_str()).ok_or("invalid fixture name")?;
        let observed = compiler::runtime::execute(TerminalRuntimeRequest {
            artifact, target: Some(target.target_name()), provisions: report.runtime_provisions(),
            provider_proposal: None, fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(fs::File::open(&input)?)), clock: None,
        }).map_err(|error| format!("{name}: {error}"))?;
        fs::write(output.join(format!("{name}.stdout")), &observed.stdout)?;
        fs::write(output.join(format!("{name}.stderr")), &observed.stderr)?;
        let TerminalExecutionStatus::ExternalCompletion(completion) = observed.status else {
            return Err(format!("{name}: incomplete execution {:?}", observed.status).into());
        };
        records.push_str(&format!("{name}\t{}\n", completion.return_code));
        fs::write(output.join("records.tsv"), &records)?;
        println!("Terminal captured {name}: exit {}, {} stdout bytes", completion.return_code, observed.stdout.len());
    }
    Ok(())
}
