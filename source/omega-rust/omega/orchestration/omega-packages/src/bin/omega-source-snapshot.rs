use omega_compiler::{
    SourceInspectionRoot, inspect_source_closure, inspect_source_closure_with_packages,
};
use std::path::PathBuf;

fn main() {
    let Some(arguments) = parse_arguments() else {
        eprintln!(
            "usage: omega-source-snapshot --repository-root <dir> [--target <name>] [--semantic-only] [--feature-census] <root.omg>"
        );
        std::process::exit(2);
    };
    match inspect(&arguments) {
        Ok(snapshot) => match if arguments.feature_census {
            snapshot.feature_census().to_json_pretty()
        } else {
            snapshot.to_json_pretty()
        } {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("cannot encode source-closure snapshot: {error}");
                std::process::exit(1);
            }
        },
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(1);
        }
    }
}

fn inspect(
    arguments: &Arguments,
) -> Result<omega_compiler::SourceClosureSnapshot, Vec<psi_diagnostics::Diagnostic>> {
    let project_root = arguments
        .root_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    if !project_root.join("build.omg").is_file() {
        return inspect_source_closure(
            &arguments.repository_root,
            &arguments.root_path,
            arguments.target_name.as_deref(),
            !arguments.semantic_only,
        );
    }
    let dependencies =
        omega_packages::extract_dependency_projection(project_root).map_err(|error| {
            vec![psi_diagnostics::Diagnostic::error(format!(
                "cannot project inspected dependencies: {error}"
            ))]
        })?;
    if dependencies.is_empty() {
        return inspect_source_closure(
            &arguments.repository_root,
            &arguments.root_path,
            arguments.target_name.as_deref(),
            !arguments.semantic_only,
        );
    }

    let entry_relative = arguments
        .root_path
        .strip_prefix(project_root)
        .map(std::path::Path::to_path_buf)
        .map_err(|_| {
            vec![psi_diagnostics::Diagnostic::error(format!(
                "inspected entry {} is outside project root {}",
                arguments.root_path.display(),
                project_root.display()
            ))]
        })?;
    let closure = omega_packages::resolve_external_local_project_closure(
        project_root,
        omega_packages::ExternalSourceContext::derive(b"omega-source-inspection-v1"),
        local_source_cache_root(),
        omega_packages::LocalSourceLimits::default(),
        omega_packages::PackageSourceClosureLimits::default(),
    )
    .map_err(|error| {
        vec![psi_diagnostics::Diagnostic::error(format!(
            "cannot resolve inspected package closure: {error}"
        ))]
    })?;
    let root_snapshot = closure.source_root(closure.graph().root()).ok_or_else(|| {
        vec![psi_diagnostics::Diagnostic::error(
            "inspected package closure lost root source custody",
        )]
    })?;
    let package_inputs =
        omega_packages::package_compilation_inputs(&closure).map_err(|errors| {
            vec![psi_diagnostics::Diagnostic::error(format!(
                "cannot construct inspected compiler package graph: {errors:?}"
            ))]
        })?;
    let identity_roots = closure
        .custodies()
        .iter()
        .filter_map(|custody| match custody.key().source_lineage() {
            omega_packages::SourceLineage::ExternalLocal(lineage) => {
                Some(SourceInspectionRoot::new(
                    custody.snapshot_root(),
                    lineage.canonical_absolute_path(),
                ))
            }
            _ => None,
        })
        .collect();
    inspect_source_closure_with_packages(
        &arguments.repository_root,
        &root_snapshot.join(entry_relative),
        arguments.target_name.as_deref(),
        !arguments.semantic_only,
        package_inputs,
        identity_roots,
    )
}

fn local_source_cache_root() -> PathBuf {
    if let Some(configured) = std::env::var_os("OMEGA_SOURCE_CACHE_DIR") {
        return PathBuf::from(configured);
    }
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(cache).join("omega/source");
    }
    if let Some(cache) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(cache).join("Omega/source");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache/omega/source");
    }
    std::env::temp_dir().join("omega-source-cache")
}

struct Arguments {
    repository_root: PathBuf,
    root_path: PathBuf,
    target_name: Option<String>,
    semantic_only: bool,
    feature_census: bool,
}

fn parse_arguments() -> Option<Arguments> {
    let mut repository_root = None;
    let mut root_path = None;
    let mut target_name = None;
    let mut semantic_only = false;
    let mut feature_census = false;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--repository-root" {
            repository_root = arguments.next().map(PathBuf::from);
            repository_root.as_ref()?;
            continue;
        }
        if argument == "--target" {
            target_name = arguments
                .next()
                .and_then(|target| target.into_string().ok());
            target_name.as_ref()?;
            continue;
        }
        if argument == "--semantic-only" {
            semantic_only = true;
            continue;
        }
        if argument == "--feature-census" {
            feature_census = true;
            continue;
        }
        if root_path.is_some() {
            return None;
        }
        root_path = Some(PathBuf::from(argument));
    }
    Some(Arguments {
        repository_root: repository_root?,
        root_path: root_path?,
        target_name,
        semantic_only,
        feature_census,
    })
}
