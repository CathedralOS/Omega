mod audit;
mod inspect_terminal;
mod output;
mod probe;
mod samples;
mod source_snapshot;

use std::path::PathBuf;

use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
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
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "source-snapshot")
    {
        source_snapshot::run(raw_arguments);
        return;
    }
    let Some(arguments) = parse_arguments() else {
        eprintln!(
            "usage: omega [--check] [--output-only] [--build-dir <dir>] [--target <name>] <root.omg>\n       omega run [--both] [--keep] [--target <name>] <root.omg>\n       omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>\n       omega audit source --kind <local|git> <locator> [--rev <rev>]\n       omega source-snapshot --repository-root <dir> [--target <name>] [--feature-census] <root.omg>\n       omega refresh-samples [samples-dir]"
        );
        std::process::exit(2);
    };

    let mut options = CompileOptions {
        build_dir: arguments.build_dir,
        root_path: arguments.root_path,
        target_name: arguments.target_name,
        write_output: false,
    };

    let artifact_policy = if arguments.output_only {
        ArtifactEmissionPolicy::OutputOnly
    } else {
        ArtifactEmissionPolicy::Full
    };
    let package_inputs = match reconcile_declared_local_dependencies(&mut options) {
        Ok(package_inputs) => package_inputs,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let build_dir = options.build_dir();
    let requested_product = if arguments.check_only {
        RequestedCompileProduct::Check
    } else {
        RequestedCompileProduct::NativeArtifact
    };
    let mut request = CompileRequest::new(options)
        .with_requested_product(requested_product)
        .with_artifact_policy(artifact_policy);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    match compile(request) {
        Ok(report) if arguments.check_only => println!("{}", report.summary()),
        Ok(report) => match output::publish_native_artifact(report, &build_dir) {
            Ok(path) => println!("published native output to {}", path.display()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        },
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }

            std::process::exit(1);
        }
    };
}

/// Enter the reconciled package path exactly when the selected project declares
/// dependencies. A dependency alias is build-owned identity, never a directory
/// under the entry source. Dependency-free standalone sources retain the small
/// direct compiler path used by focused probes and canaries.
fn reconcile_declared_local_dependencies(
    options: &mut CompileOptions,
) -> Result<Option<omega_package_compilation::PackageCompilationInputs>, String> {
    let project_root = options
        .root_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    if !project_root.join("build.omg").is_file() {
        return Ok(None);
    }
    let dependencies = omega_package_manager::extract_dependency_projection(&project_root)
        .map_err(|error| format!("cannot project declared dependencies: {error}"))?;
    if dependencies.is_empty() {
        return Ok(None);
    }

    let entry_relative = options
        .root_path
        .strip_prefix(&project_root)
        .map(std::path::Path::to_path_buf)
        .map_err(|_| {
            format!(
                "entry source {} is outside its selected project root {}",
                options.root_path.display(),
                project_root.display()
            )
        })?;
    let storage = local_source_storage()?;
    let closure = omega_package_manager::resolve_external_local_project_closure_with_storage(
        &project_root,
        omega_package_manager::ExternalSourceContext::derive(b"omega-local-project-v1"),
        &storage,
        omega_package_manager::LocalSourceLimits::default(),
        omega_package_manager::PackageSourceClosureLimits::default(),
    )
    .map_err(|error| format!("cannot resolve declared package closure: {error}"))?;
    let root_snapshot = closure
        .source_root(closure.graph().root())
        .ok_or_else(|| "resolved package closure lost its root source custody".to_owned())?;
    options.root_path = root_snapshot.join(entry_relative);
    omega_package_manager::package_compilation_inputs(&closure)
        .map(Some)
        .map_err(|errors| format!("cannot construct compiler package graph: {errors:?}"))
}

fn local_source_storage() -> Result<omega_package_manager::SourceResolverStorage, String> {
    omega_package_manager::SourceResolverStorage::for_current_user()
        .map_err(|error| format!("cannot open private source resolver storage: {error}"))
}

struct CliArguments {
    build_dir: Option<PathBuf>,
    check_only: bool,
    output_only: bool,
    root_path: PathBuf,
    target_name: Option<String>,
}

fn parse_arguments() -> Option<CliArguments> {
    let mut build_dir = None;
    let mut check_only = false;
    let mut output_only = false;
    let mut root_path = None;
    let mut target_name = None;
    let mut arguments = std::env::args_os().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == "--check" {
            check_only = true;
            continue;
        }

        if argument == "--output-only" {
            output_only = true;
            continue;
        }

        if argument == "--build-dir" {
            build_dir = arguments.next().map(PathBuf::from);
            build_dir.as_ref()?;
            continue;
        }

        if argument == "--target" {
            target_name = arguments
                .next()
                .and_then(|target_name| target_name.into_string().ok());
            target_name.as_ref()?;
            continue;
        }

        if root_path.is_some() {
            return None;
        }

        root_path = Some(PathBuf::from(argument));
    }

    Some(CliArguments {
        build_dir,
        check_only,
        output_only,
        root_path: root_path?,
        target_name,
    })
}
