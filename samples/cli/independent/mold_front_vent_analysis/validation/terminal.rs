//! Private exact-output acceptance consumer of the existing public product APIs.
//! Compile through ordinary accepted package preparation, then execute the
//! retained canonical Terminal artifact and its original runtime provisions.
//! No checked-tree fallback, invented entry/authority, or compiler export change.
use std::{fs, io::Cursor, path::PathBuf};
use compiler::runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use terminal_interpreter::TerminalExecutionStatus;

fn validate(root: PathBuf) -> Result<(), String> {
    let target = target::TargetProfile::LinuxX64;
    let prepared = prepare_local_project(&root.join("main.omg"), LocalProjectPreparationOptions {
        target, offline: false,
    }).map_err(|error| error.to_string())?.ok_or("expected a package project")?;
    let build = root.join("build/terminal-acceptance");
    let report = compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, build, target)
    ).map_err(|error| error.to_string())?;
    let artifact = report.retained_native_artifact().map(|a| a.psi_artifact())
        .or_else(|| report.retained_dynamic_elf_artifact().map(|a| a.psi_artifact()))
        .ok_or("native product lost its canonical Terminal artifact")?;
    let output_dir = root.join("build/validation/terminal");
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    let mut paths: Vec<_> = fs::read_dir(root.join("data")).map_err(|error| error.to_string())?
        .map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|e|e=="in")).collect();
    let tiny_paths: Vec<_> = fs::read_dir(root.join("build/tiny-fixtures")).map_err(|e|e.to_string())?
        .map(|entry|entry.unwrap().path()).filter(|path|path.extension().is_some_and(|e|e=="in")).collect();
    if tiny_paths.len() != 619 || paths.len() != 22 {
        return Err(format!("expected 619 tiny and 22 selected/invalid fixtures, found {} and {}",
            tiny_paths.len(), paths.len()));
    }
    paths.extend(tiny_paths);
    paths.sort();
    let mut count = 0;
    for path in paths {
        let name = path.file_stem().unwrap().to_str().unwrap();
        let invalid = name.starts_with("invalid_");
        let input = fs::read(&path).map_err(|error| error.to_string())?;
        let expected = if invalid {b"INVALID_INPUT\n".to_vec()} else {
            fs::read(path.with_extension("expected")).map_err(|error| error.to_string())?
        };
        let observed = compiler::runtime::execute(TerminalRuntimeRequest {
            artifact, target: Some("linux_x86_64"), provisions: report.runtime_provisions(),
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input))), clock: None,
        }).map_err(|error| format!("{name}: {error}"))?;
        let code = match observed.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            other => return Err(format!("{name}: unexpected Terminal status {other:?}")),
        };
        fs::write(output_dir.join(format!("{name}.stdout")), &observed.stdout).map_err(|e|e.to_string())?;
        fs::write(output_dir.join(format!("{name}.stderr")), &observed.stderr).map_err(|e|e.to_string())?;
        if code != if invalid {64} else {0} || observed.stdout != expected || !observed.stderr.is_empty() {
            return Err(format!("{name}: exit={code}, stdout={:?}, stderr={:?}", observed.stdout, observed.stderr));
        }
        println!("Terminal exact-output OK {name} exit={code} bytes={}", observed.stdout.len());
        count += 1;
    }
    println!("Terminal accepted {count} fixtures; byte tolerance=0");
    Ok(())
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("application directory"));
    let result = std::thread::Builder::new().stack_size(64 * 1024 * 1024)
        .spawn(move || validate(root)).unwrap().join().unwrap();
    if let Err(error) = result {eprintln!("{error}"); std::process::exit(1);}
}
