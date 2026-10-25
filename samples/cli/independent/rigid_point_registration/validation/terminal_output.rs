//! Auxiliary output observer over the shipped complete-project/runtime APIs.
//! Compiles the actual admitted application once; every fixture consumes its
//! retained canonical Terminal artifact and exact runtime provisions. No tree
//! interpretation, synthetic receiver, reconstructed artifact or trust bypass.
use std::{fs, path::PathBuf};
use package_manager::operations::{prepare_local_project, LocalProjectPreparationOptions,
    PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native};
use terminal_runtime::{TerminalRuntimeRequest, TerminalDomainExitDelegation};
use terminal_interpreter::TerminalExecutionStatus;

fn run(root: PathBuf, output: PathBuf, fuel: u64, timeout: u64, names: Vec<String>) -> Result<(), String> {
    fs::create_dir_all(&output).map_err(|error|error.to_string())?;
    let prepared=prepare_local_project(&root.join("main.omg"), LocalProjectPreparationOptions {
        target:target::TargetProfile::LinuxX64, offline:true
    }).map_err(|error|error.to_string())?.ok_or("expected admitted complete application")?;
    let report=compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared, output.join("compiled"), target::TargetProfile::LinuxX64
    )).map_err(|error|error.to_string())?;
    let artifact=report.retained_native_artifact().map(|artifact|artifact.psi_artifact())
        .or_else(||report.retained_dynamic_elf_artifact().map(|artifact|artifact.psi_artifact()))
        .ok_or("native report has no retained canonical Terminal artifact")?;
    for name in names {
        if !name.bytes().all(|byte|byte.is_ascii_alphanumeric()||byte==b'_') {
            return Err("invalid fixture name".into());
        }
        println!("starting {name}; sponsor fuel {fuel}");
        let started=std::time::Instant::now();
        let (completed,watch)=std::sync::mpsc::channel();
        let watched_name=name.clone();
        let watchdog=std::thread::spawn(move|| {
            if matches!(watch.recv_timeout(std::time::Duration::from_secs(timeout)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)) {
                eprintln!("{watched_name}: Terminal wall-clock allowance exhausted");
                std::process::exit(124);
            }
        });
        let observed=terminal_runtime::execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact,target:Some("linux_x86_64"),provisions:report.runtime_provisions(),provider_proposal:None,
            fuel_allowance:fuel,domain_exit:TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input:Some(Box::new(fs::File::open(root.join("data").join(format!("{name}.txt")))
                .map_err(|error|error.to_string())?)),clock:None,
        }).map_err(|error|format!("{name}: runtime declined/failed: {error}"))?;
        fs::write(output.join(format!("{name}.terminal.stdout")),&observed.stdout).map_err(|error|error.to_string())?;
        fs::write(output.join(format!("{name}.terminal.stderr")),&observed.stderr).map_err(|error|error.to_string())?;
        fs::write(output.join(format!("{name}.terminal.status")),format!("{:?}\n",observed.status)).map_err(|error|error.to_string())?;
        let code=match observed.status {
            TerminalExecutionStatus::ExternalCompletion(completion)=>completion.return_code,
            status=>return Err(format!("{name}: no completed program exit: {status:?}")),
        };
        fs::write(output.join(format!("{name}.terminal.exit")),format!("{code}\n")).map_err(|error|error.to_string())?;
        completed.send(()).map_err(|error|error.to_string())?;
        watchdog.join().map_err(|_|"watchdog panicked")?;
        println!("completed {name}; exit {code}; elapsed {:?}; stdout {} bytes",started.elapsed(),observed.stdout.len());
    }
    Ok(())
}
fn main() {
    let mut arguments=std::env::args().skip(1);
    let parsed=(||->Result<_,String>{
        let root=PathBuf::from(arguments.next().ok_or("missing application root")?).canonicalize().map_err(|error|error.to_string())?;
        let output=PathBuf::from(arguments.next().ok_or("missing output directory")?);
        let fuel=arguments.next().ok_or("missing sponsor fuel")?.parse::<u64>().map_err(|error|error.to_string())?;
        let timeout=arguments.next().ok_or("missing per-case wall-clock allowance")?.parse::<u64>().map_err(|error|error.to_string())?;
        if fuel==0 || timeout==0 {return Err("fuel and timeout must be positive".into());}
        let names=arguments.collect::<Vec<_>>();if names.is_empty(){return Err("missing fixture names".into());}
        Ok((root,output,fuel,timeout,names))
    })();
    let (root,output,fuel,timeout,names)=match parsed {Ok(value)=>value,Err(error)=>{eprintln!("{error}");std::process::exit(2)}};
    let thread=std::thread::Builder::new().name("rigid-registration-terminal-output".into()).stack_size(256*1024*1024)
        .spawn(move||run(root,output,fuel,timeout,names));
    let result=match thread {Ok(thread)=>match thread.join(){Ok(result)=>result,Err(_)=>Err("validation worker panicked".into())},Err(error)=>Err(error.to_string())};
    if let Err(error)=result {eprintln!("{error}");std::process::exit(1);}
}
