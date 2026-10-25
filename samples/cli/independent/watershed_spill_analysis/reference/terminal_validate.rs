// Auxiliary validator over stable public complete-project and runtime APIs.
// It compiles the real Omega application once, retains that exact Terminal
// artifact and provider provisions, then checks both engines for every fixture.
// No checked-tree interpretation, artifact reconstruction or trust bypass.
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}};
use package_manager::operations::{prepare_local_project, LocalProjectPreparationOptions,
    PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native};
use terminal_runtime::{TerminalRuntimeRequest, TerminalDomainExitDelegation, TerminalClockSnapshot};
use terminal_interpreter::TerminalExecutionStatus;

fn run(root: PathBuf) -> Result<(), String> {
    let output=root.join("build/validation/bulk");
    fs::create_dir_all(&output).map_err(|e|e.to_string())?;
    let prepared=prepare_local_project(&root.join("main.omg"), LocalProjectPreparationOptions {
        target:target::TargetProfile::LinuxX64, offline:true
    }).map_err(|e|e.to_string())?.ok_or("expected complete package project")?;
    let report=compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared, output.join("native"), target::TargetProfile::LinuxX64
    )).map_err(|e|e.to_string())?;
    let mut fixtures:Vec<PathBuf>=fs::read_dir(root.join("fixtures")).map_err(|e|e.to_string())?
        .map(|entry|entry.unwrap().path()).filter(|p|p.extension().is_some_and(|x|x=="txt")
            && !p.file_name().unwrap().to_string_lossy().ends_with(".expected.txt")).collect();
    fixtures.sort();
    let artifact=report.retained_native_artifact().map(|a|a.psi_artifact())
        .or_else(||report.retained_dynamic_elf_artifact().map(|a|a.psi_artifact()))
        .ok_or("native report has no retained Terminal artifact")?;
    for fixture in &fixtures {
        let name=fixture.file_stem().unwrap().to_string_lossy();
        let expected=fs::read(root.join("fixtures").join(format!("{name}.expected.txt"))).map_err(|e|e.to_string())?;
        let expected_code=if name.starts_with("invalid_"){2}else{0};
        let observed=terminal_runtime::execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact, target:Some("linux_x86_64"), provisions:report.runtime_provisions(),
            // Ordinary CLI default: no root-authored Terminal provider proposal.
            provider_proposal:None,
            fuel_allowance:10_000_000,
            domain_exit:TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input:Some(Box::new(fs::File::open(fixture).map_err(|e|e.to_string())?)),
            clock:Some(TerminalClockSnapshot::capture_host_epoch().map_err(|e|e.to_string())?),
        }).map_err(|e|format!("{name}: Terminal declined/failed: {e}"))?;
        fs::write(output.join(format!("{name}.terminal.stdout")),&observed.stdout).map_err(|e|e.to_string())?;
        fs::write(output.join(format!("{name}.terminal.stderr")),&observed.stderr).map_err(|e|e.to_string())?;
        let code=match observed.status {
            TerminalExecutionStatus::ExternalCompletion(completion)=>completion.return_code,
            other=>return Err(format!("{name}: unexpected Terminal status {other:?}")),
        };
        fs::write(output.join(format!("{name}.terminal.exit")),format!("{code}\n")).map_err(|e|e.to_string())?;
        if code!=expected_code || observed.stdout!=expected || !observed.stderr.is_empty() {
            return Err(format!("{name}: Terminal output/exit oracle mismatch"));
        }
        println!("{name}: Terminal stdout, stderr and exit exact");
    }
    // Publication owns the actual executable; no alternate emitter or native
    // reconstruction is involved. Every process gets a fresh finite stdin file.
    let (_,executable)=omega_build::publication::publish_native_artifact(report,&output.join("native"))?;
    fs::write(output.join("executable-path.txt"),format!("{}\n",executable.display())).map_err(|e|e.to_string())?;
    for fixture in &fixtures {
        let name=fixture.file_stem().unwrap().to_string_lossy();
        let expected=fs::read(root.join("fixtures").join(format!("{name}.expected.txt"))).map_err(|e|e.to_string())?;
        let expected_code=if name.starts_with("invalid_"){2}else{0};
        let observed=Command::new(&executable).stdin(Stdio::from(fs::File::open(fixture).map_err(|e|e.to_string())?))
            .output().map_err(|e|e.to_string())?;
        fs::write(output.join(format!("{name}.native.stdout")),&observed.stdout).map_err(|e|e.to_string())?;
        fs::write(output.join(format!("{name}.native.stderr")),&observed.stderr).map_err(|e|e.to_string())?;
        let code=observed.status.code().ok_or_else(||format!("{name}: native signal death"))?;
        fs::write(output.join(format!("{name}.native.exit")),format!("{code}\n")).map_err(|e|e.to_string())?;
        if code!=expected_code || observed.stdout!=expected || !observed.stderr.is_empty() {
            return Err(format!("{name}: native output/exit oracle mismatch"));
        }
        println!("{name}: native stdout, stderr and exit exact");
    }
    println!("{} frozen fixtures, both engines byte-exact",fixtures.len());
    Ok(())
}
fn main(){
    let root=Path::new(&std::env::args().nth(1).expect("sample directory")).canonicalize().unwrap();
    let result=std::thread::Builder::new().name("watershed-validation".into())
        .stack_size(256*1024*1024).spawn(move||run(root)).unwrap().join().unwrap();
    if let Err(error)=result {eprintln!("{error}");std::process::exit(1);}
}
