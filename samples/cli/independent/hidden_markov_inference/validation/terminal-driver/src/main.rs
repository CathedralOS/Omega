//! Private observation consumer of the ordinary accepted native product.
//! Run never changes package decisions. Explicit review/resume modes use the
//! ordinary manager workflow for this exact executable's compiler identity.
//! No fixture trust, compiler-identity override or source interpreter is used.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest, execute};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use package_manager::{
    PackageBuildSession, PackageCommand, PackageCommandKind, PackageCommandOptions,
    PackageCommandStatus, execute_package_command,
};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf};
use terminal_interpreter::TerminalExecutionStatus;
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    std::thread::Builder::new()
        .name("hmm-validation".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(validate)?
        .join()
        .map_err(|_| "validation worker panicked")?
}
fn validate() -> Result<(), Box<dyn Error + Send + Sync>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 2 {
        return Err("usage: hmm-terminal-validation SAMPLE_DIRECTORY OUTPUT_DIRECTORY | --review SAMPLE_DIRECTORY | --resume-review SAMPLE_DIRECTORY".into());
    }
    let review = arguments[0] == "--review" || arguments[0] == "--resume-review";
    let root = PathBuf::from(&arguments[usize::from(review)]).canonicalize()?;
    let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64"))
        .map_err(|diagnostic| diagnostic.message)?;
    if review {
        let resume = arguments[0] == "--resume-review";
        let command = if resume {
            PackageCommand::Resume {
                kind: PackageCommandKind::Update,
            }
        } else {
            PackageCommand::Update {
                packages: Vec::new(),
                revision: None,
            }
        };
        let options = PackageCommandOptions {
            project_root: root.clone(),
            targets: if resume { Vec::new() } else { vec![profile] },
            offline: true,
            build_inputs: None,
        };
        let mut session = PackageBuildSession::create(&root.join("build/package-manager"))
            .map_err(|error| error.to_string())?;
        let result = execute_package_command(command, options, None, &mut session);
        let outcome = session.dispose(result).map_err(|error| error.to_string())?;
        print!("{}", outcome.report);
        for path in outcome.review_paths {
            println!("review: {}", path.display());
        }
        if outcome.status == PackageCommandStatus::ReviewRequired {
            std::process::exit(3);
        }
        return Ok(());
    }
    let output = PathBuf::from(&arguments[1]);
    fs::create_dir_all(&output)?;
    if !root.join("omega.lock").is_file() {
        return Err(
            "an already accepted omega.lock for this exact compiler executable is required".into(),
        );
    }
    let prepared = prepare_local_project(
        &root.join("main.omg"),
        LocalProjectPreparationOptions {
            target: profile,
            offline: true,
        },
    )
    .map_err(|error| error.to_string())?
    .ok_or("the managed sample package was not prepared")?;
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        output.join("native-build"),
        profile,
    ))
    .map_err(|error| error.to_string())?;
    let artifact = report
        .retained_native_artifact()
        .map(|artifact| artifact.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|artifact| artifact.psi_artifact())
        })
        .ok_or("the accepted native report did not retain its exact Terminal artifact")?;
    fs::write(output.join("application.terminal"), artifact.to_bytes())?;
    let mut fixtures: Vec<_> = fs::read_dir(root.join("data"))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "stdin")
        })
        .collect();
    fixtures.sort();
    let mut sources = String::new();
    for name in [
        "main.omg",
        "build.omg",
        "domain/inference.omg",
        "domain/viterbi.omg",
        "io/trace.omg",
    ] {
        sources.push_str(&format!(
            "{name}\t{}\n",
            digest(&fs::read(root.join(name))?)
        ));
    }
    fs::write(output.join("sources.tsv"), sources)?;
    fs::write(output.join("terminal.sha256"), digest(&artifact.to_bytes()))?;
    fs::write(
        output.join("producer.sha256"),
        digest(&fs::read(std::env::current_exe()?)?),
    )?;
    let mut records = String::from("case\texit\tinput_sha256\tstdout_sha256\tstderr_sha256\n");
    for fixture in fixtures {
        let name = fixture
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("invalid case name")?;
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err("unexpected case name".into());
        }
        let observation = execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            provider_proposal: None,
            artifact,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            fuel_allowance: 100_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(fs::File::open(&fixture)?)),
            clock: None,
        })
        .map_err(|error| format!("{name}: {error:?}"))?;
        fs::write(output.join(format!("{name}.stdout")), &observation.stdout)?;
        fs::write(output.join(format!("{name}.stderr")), &observation.stderr)?;
        let TerminalExecutionStatus::ExternalCompletion(completion) = observation.status else {
            return Err(format!(
                "{name}: expected the app's explicit Console exit, observed {:?}",
                observation.status
            )
            .into());
        };
        records.push_str(&format!(
            "{name}\t{}\t{}\t{}\t{}\n",
            completion.return_code,
            digest(&fs::read(&fixture)?),
            digest(&observation.stdout),
            digest(&observation.stderr)
        ));
        fs::write(output.join("records.tsv"), &records)?;
        println!("observed Terminal {name} exit {}", completion.return_code);
    }
    let published =
        compilation_products::publication::publish_compilation(report, &output.join("native"))?;
    let executable = published
        .checked_native_executable_path()
        .ok_or("native publication did not retain executable custody")?;
    fs::write(
        output.join("native.sha256"),
        digest(&fs::read(&executable)?),
    )?;
    fs::write(
        output.join("native-path.txt"),
        executable.display().to_string(),
    )?;
    println!("retained native executable {}", executable.display());
    Ok(())
}
