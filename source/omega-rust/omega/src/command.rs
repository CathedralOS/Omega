mod audit;
mod inspect_terminal;
mod output;
mod probe;
mod samples;

use std::path::PathBuf;

use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, OptimizationRollback,
    RequestedCompileProduct, compile,
};
use omega_core::allocations::CountingAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator::system();

pub(crate) fn run() {
    // `omega refresh-samples [samples-dir]`: compile every sample main.omg
    // under samples/, in place, in parallel, so each sample folder holds a
    // current, runnable build/omega-program.exe. Cross-platform (no shell
    // script) and links this binary's own compiler. This package is a member of
    // the root Cargo workspace.
    let mut raw_arguments = std::env::args_os().skip(1);
    let first_argument = raw_arguments.next();
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "refresh-samples")
    {
        let samples_root = raw_arguments
            .next()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("samples"));
        samples::refresh(&samples_root);
    }
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "run")
    {
        probe::run(raw_arguments);
    }
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "inspect-terminal")
    {
        inspect_terminal::run(raw_arguments);
        return;
    }
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "audit")
    {
        audit::run(raw_arguments);
        return;
    }
    let arguments = parse_arguments().unwrap_or_else(|error| {
        if !error.is_empty() {
            eprintln!("{error}");
        }
        eprintln!("{}", usage());
        std::process::exit(2);
    });

    let mut options = CompileOptions {
        build_dir: arguments.build_dir,
        root_path: arguments.root_path,
        target_name: arguments.target_name,
    };
    // Artifact placement belongs to the user-authored entrypoint. Package
    // preparation may replace only the compilation root with an immutable
    // resolver snapshot.
    let build_dir = options.retain_build_dir();
    let policy_root_path = options.root_path.clone();

    let artifact_policy = if arguments.output_only {
        ArtifactEmissionPolicy::OutputOnly
    } else {
        ArtifactEmissionPolicy::Full
    };
    let package_inputs = match omega_package_manager::operations::prepare_local_project(
        &options.root_path,
        options.target_name.as_deref(),
    ) {
        Ok(Some(prepared)) => {
            let (entry_path, package_inputs) = prepared.into_parts();
            options.root_path = entry_path;
            Some(package_inputs)
        }
        Ok(None) => None,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let requested_product = if arguments.check_only {
        RequestedCompileProduct::Check
    } else {
        RequestedCompileProduct::NativeArtifact
    };
    let mut request = CompileRequest::new(options)
        .with_requested_product(requested_product)
        .with_artifact_policy(artifact_policy)
        .with_optimization_rollback(arguments.optimization_rollback);
    let accepted_admissions = match omega_trust_ledger::read_trust_admissions(&policy_root_path) {
        Ok(admissions) => admissions,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(1);
        }
    };
    request = request.with_accepted_trust_admissions(accepted_admissions);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    match compile(request) {
        Ok(report) => {
            let settlement = report.trust_admission_settlement();
            if arguments.accept_admissions {
                if let Err(diagnostics) = omega_trust_ledger::accept_trust_admissions(
                    &policy_root_path,
                    settlement.required(),
                ) {
                    for diagnostic in diagnostics {
                        eprintln!("{diagnostic}");
                    }
                    std::process::exit(1);
                }
            } else if !settlement.is_exactly_admitted() {
                for admission in settlement.unresolved() {
                    eprintln!(
                        "unresolved trust admission `{}` [{}]{}",
                        admission.commitment(),
                        admission.digest(),
                        admission
                            .report_identity()
                            .map(|identity| format!(" (report {identity:016x})"))
                            .unwrap_or_default(),
                    );
                }
                for admission in settlement.unused() {
                    eprintln!(
                        "stale trust admission `{}` [{}]{}",
                        admission.commitment(),
                        admission.digest(),
                        admission
                            .report_identity()
                            .map(|identity| format!(" (report {identity:016x})"))
                            .unwrap_or_default(),
                    );
                }
                eprintln!("run again with --accept-admissions to accept this exact set");
                std::process::exit(1);
            }
            if arguments.check_only {
                println!("{}", report.summary());
            } else {
                match output::publish_native_artifact(report, &build_dir) {
                    Ok((published, path)) => {
                        if let Some(receipt) = published.optimization_rollback_receipt() {
                            println!("optimizer rollback: {receipt}");
                        }
                        println!("published native output to {}", path.display());
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }

            std::process::exit(1);
        }
    };
}

struct CliArguments {
    accept_admissions: bool,
    build_dir: Option<PathBuf>,
    check_only: bool,
    output_only: bool,
    root_path: PathBuf,
    target_name: Option<String>,
    optimization_rollback: OptimizationRollback,
}

fn usage() -> &'static str {
    "usage: omega [--check] [--accept-admissions] [--output-only] [--build-dir <dir>] [--target <name>] [--disable-optimization <ExactName>]... <root.omg>\n       omega run [--both] [--keep] [--target <name>] <root.omg>\n       omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>\n       omega audit source --kind <local|git> <locator> [--rev <rev>]\n       omega refresh-samples [samples-dir]"
}

fn parse_arguments() -> Result<CliArguments, String> {
    let mut accept_admissions = false;
    let mut build_dir = None;
    let mut check_only = false;
    let mut disabled_optimizations = Vec::new();
    let mut output_only = false;
    let mut root_path = None;
    let mut target_name = None;
    let mut arguments = std::env::args_os().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == "--check" {
            check_only = true;
            continue;
        }

        if argument == "--accept-admissions" {
            accept_admissions = true;
            continue;
        }

        if argument == "--output-only" {
            output_only = true;
            continue;
        }

        if argument == "--build-dir" {
            build_dir = arguments.next().map(PathBuf::from);
            if build_dir.is_none() {
                return Err("--build-dir requires a directory".into());
            }
            continue;
        }

        if argument == "--target" {
            target_name = arguments
                .next()
                .and_then(|target_name| target_name.into_string().ok());
            if target_name.is_none() {
                return Err("--target requires a UTF-8 target name".into());
            }
            continue;
        }

        if argument == "--disable-optimization" {
            let Some(name) = arguments.next() else {
                return Err("--disable-optimization requires one exact optimization name".into());
            };
            let name = name.into_string().map_err(|_| {
                "--disable-optimization requires a UTF-8 exact optimization name".to_owned()
            })?;
            disabled_optimizations.push(name);
            continue;
        }

        if root_path.is_some() {
            return Err(format!(
                "unexpected extra argument `{}`",
                argument.to_string_lossy()
            ));
        }

        root_path = Some(PathBuf::from(argument));
    }

    let optimization_rollback =
        OptimizationRollback::from_exact_names(disabled_optimizations.iter().map(String::as_str))
            .map_err(|error| error.to_string())?;
    Ok(CliArguments {
        accept_admissions,
        build_dir,
        check_only,
        output_only,
        root_path: root_path.ok_or_else(|| "missing root Omega source path".to_owned())?,
        target_name,
        optimization_rollback,
    })
}
