mod audit;
mod inspect_terminal;
mod output;
mod package;
mod probe;
mod samples;

use std::path::PathBuf;

use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, OptimizationRollback,
    RequestedCompileProduct, compile,
};
use core::allocations::CountingAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator::system();

/// Recursive parsing and representation walks must reach their explicit depth
/// guards on hosts whose default thread stacks are small, and Windows gives the
/// main thread one mebibyte. Compiler entry points already run their work on a
/// large stack, but native realization is reached on the caller's thread, so the
/// primary `omega <root.omg>` route ran the deepest walks on the smallest stack
/// and aborted before writing an artifact. Every subcommand crosses here, so one
/// thread at this boundary covers all of them.
const COMMAND_STACK_SIZE: usize = 256 * 1024 * 1024;

pub(crate) fn run() {
    std::thread::Builder::new()
        .name("omega-command".to_owned())
        .stack_size(COMMAND_STACK_SIZE)
        .spawn(dispatch)
        .expect("failed to spawn command thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

fn dispatch() {
    // `omega refresh-samples [samples-dir]`: compile every sample main.omg
    // under samples/, in place, in parallel, so each sample folder holds a
    // current, runnable build/omega-program.exe. Cross-platform (no shell
    // script) and links this binary's own compiler. This package is a member of
    // the root Cargo workspace.
    let mut raw_arguments = std::env::args_os().skip(1);
    let first_argument = raw_arguments.next();
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "install" || first == "update")
    {
        let kind = if first_argument
            .as_deref()
            .is_some_and(|first| first == "install")
        {
            package_manager::operations::PackageCommandKind::Install
        } else {
            package_manager::operations::PackageCommandKind::Update
        };
        package::run(kind, raw_arguments);
        return;
    }
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
    let target_profile =
        target::TargetProfile::from_omega_target_name(options.target_name.as_deref())
            .unwrap_or_else(|diagnostic| {
                eprintln!("{diagnostic}");
                std::process::exit(1);
            });
    let prepared_project = match package_manager::operations::prepare_local_project_for_target(
        &options.root_path,
        target_profile,
    ) {
        Ok(prepared) => prepared,
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
    let accepted_admissions = match trust_ledger::read_trust_admissions(&policy_root_path) {
        Ok(admissions) => admissions,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(1);
        }
    };
    let report = match prepared_project {
        Some(prepared) if !arguments.check_only => {
            let policy = arguments
                .package_root_policy
                .as_deref()
                .map(open_package_root_policy)
                .transpose()
                .unwrap_or_else(|error| {
                    eprintln!("{error}");
                    std::process::exit(1);
                });
            let mut request = package_manager::operations::PreparedLocalProjectNativeRequest::new(
                prepared,
                &build_dir,
                target_profile,
            )
            .with_artifact_policy(artifact_policy)
            .with_accepted_trust_admissions(accepted_admissions)
            .with_optimization_rollback(arguments.optimization_rollback);
            if let Some((directory, name)) = policy.as_ref() {
                request = request.with_root_policy(
                    package_manager::operations::LocalProjectRootPolicy::new(directory, name),
                );
            }
            package_manager::operations::compile_prepared_local_project_for_native(request)
                .unwrap_or_else(|error| {
                    eprintln!("{error}");
                    std::process::exit(1);
                })
        }
        Some(prepared) => {
            if arguments.package_root_policy.is_some() {
                eprintln!(
                    "--package-root-policy requires native production from a build.omg project"
                );
                std::process::exit(1);
            }
            if !arguments.optimization_rollback.is_empty() {
                let names = arguments
                    .optimization_rollback
                    .requested_disabled()
                    .as_slice()
                    .iter()
                    .map(|optimization| format!("`{}`", optimization.build_case_name()))
                    .collect::<Vec<_>>()
                    .join(", ");
                eprintln!(
                    "optimization rollback {names} requires NativeArtifact production; Check does not enter native optimizer realization"
                );
                std::process::exit(1);
            }
            let request = package_manager::operations::PreparedLocalProjectCheckRequest::new(
                prepared,
                &build_dir,
                target_profile,
            )
            .with_artifact_policy(artifact_policy)
            .with_accepted_trust_admissions(accepted_admissions);
            package_manager::operations::check_prepared_local_project(request).unwrap_or_else(
                |error| {
                    eprintln!("{error}");
                    std::process::exit(1);
                },
            )
        }
        None => {
            if arguments.package_root_policy.is_some() {
                eprintln!(
                    "--package-root-policy requires native production from a build.omg project"
                );
                std::process::exit(1);
            }
            let request = CompileRequest::new(options)
                .with_requested_product(requested_product)
                .with_artifact_policy(artifact_policy)
                .with_optimization_rollback(arguments.optimization_rollback)
                .with_accepted_trust_admissions(accepted_admissions);
            compile(request).unwrap_or_else(|diagnostics| {
                for diagnostic in diagnostics {
                    eprintln!("{diagnostic}");
                }
                std::process::exit(1);
            })
        }
    };
    let settlement = report.trust_admission_settlement();
    if arguments.accept_admissions {
        if let Err(diagnostics) =
            trust_ledger::accept_trust_admissions(&policy_root_path, settlement.required())
        {
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

fn open_package_root_policy(
    path: &std::path::Path,
) -> Result<
    (
        package_manager::review::ReviewOnlyRootPolicyDirectory,
        package_manager::review::ReviewOnlyRootPolicyName,
    ),
    String,
> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "--package-root-policy requires a UTF-8 direct-child filename".to_owned())?;
    let name = package_manager::review::ReviewOnlyRootPolicyName::parse(name)
        .map_err(|error| error.to_string())?;
    let directory_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let directory =
        cap_std::fs::Dir::open_ambient_dir(directory_path, cap_std::ambient_authority()).map_err(
            |error| {
                format!(
                    "cannot open explicit package root-policy directory {}: {error}",
                    directory_path.display()
                )
            },
        )?;
    let directory = package_manager::review::ReviewOnlyRootPolicyDirectory::from_capability(
        directory,
        directory_path,
    )
    .map_err(|error| error.to_string())?;
    Ok((directory, name))
}

struct CliArguments {
    accept_admissions: bool,
    build_dir: Option<PathBuf>,
    check_only: bool,
    output_only: bool,
    package_root_policy: Option<PathBuf>,
    root_path: PathBuf,
    target_name: Option<String>,
    optimization_rollback: OptimizationRollback,
}

fn usage() -> &'static str {
    "usage: omega [--check] [--accept-admissions] [--output-only] [--package-root-policy <file>] [--build-dir <dir>] [--target <name>] [--disable-optimization <ExactName>]... <root.omg>\n       omega run [--both] [--keep] [--target <name>] <root.omg>\n       omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>\n       omega audit source --kind <local|git> <locator> [--rev <rev>]\n       omega install <source> [--rev <revision>] [--package <declared-name>] [--as <alias>] [--target <name>]... [--project <dir>]\n       omega update [package-or-alias...] [--to <revision>] [--target <name>]... [--project <dir>]\n       omega install|update --resume [--project <dir>]\n       omega install|update --discard-review [--project <dir>]\n       omega refresh-samples [samples-dir]"
}

fn parse_arguments() -> Result<CliArguments, String> {
    let mut accept_admissions = false;
    let mut build_dir = None;
    let mut check_only = false;
    let mut disabled_optimizations = Vec::new();
    let mut output_only = false;
    let mut package_root_policy = None;
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

        if argument == "--package-root-policy" {
            package_root_policy = arguments.next().map(PathBuf::from);
            if package_root_policy.is_none() {
                return Err("--package-root-policy requires a file".into());
            }
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

        // Falling through would assign this as the root path, so a misspelled flag
        // surfaced as a package-resolution failure against a directory that does not
        // exist, rather than as a rejected option.
        if argument.to_string_lossy().starts_with("--") {
            return Err(format!(
                "unrecognized option `{}`",
                argument.to_string_lossy()
            ));
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
        package_root_policy,
        root_path: root_path.ok_or_else(|| "missing root Omega source path".to_owned())?,
        target_name,
        optimization_rollback,
    })
}
