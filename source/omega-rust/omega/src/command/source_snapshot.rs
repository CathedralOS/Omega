use omega_compiler::{
    PackageSourceClosureCustodySnapshot, SourceInspectionRoot, inspect_source_closure,
    inspect_source_closure_with_packages,
};
use std::ffi::OsString;
use std::path::PathBuf;

const USAGE: &str = "usage: omega source-snapshot --repository-root <dir> [--target <name>] [--feature-census] <root.omg>";

pub(super) fn run(arguments: impl Iterator<Item = OsString>) {
    let Some(arguments) = parse_arguments(arguments) else {
        eprintln!("{USAGE}");
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
    let storage = super::local_source_storage()
        .map_err(|error| vec![psi_diagnostics::Diagnostic::error(error)])?;
    inspect_with_storage(arguments, &storage)
}

fn inspect_with_storage(
    arguments: &Arguments,
    storage: &omega_package_manager::SourceResolverStorage,
) -> Result<omega_compiler::SourceClosureSnapshot, Vec<psi_diagnostics::Diagnostic>> {
    let root_path = arguments.root_path.canonicalize().map_err(|error| {
        vec![psi_diagnostics::Diagnostic::error(format!(
            "cannot canonicalize inspected entry {}: {error}",
            arguments.root_path.display()
        ))]
    })?;
    let project_root = root_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    if !project_root.join("build.omg").is_file() {
        return inspect_source_closure(
            &arguments.repository_root,
            &root_path,
            arguments.target_name.as_deref(),
        );
    }
    let entry_relative = root_path
        .strip_prefix(project_root)
        .map(std::path::Path::to_path_buf)
        .map_err(|_| {
            vec![psi_diagnostics::Diagnostic::error(format!(
                "inspected entry {} is outside project root {}",
                root_path.display(),
                project_root.display()
            ))]
        })?;
    let closure = omega_package_manager::resolve_external_local_project_closure_with_storage(
        project_root,
        omega_package_manager::ExternalSourceContext::derive(b"omega-source-inspection-v1"),
        storage,
        omega_package_manager::LocalSourceLimits::default(),
        omega_package_manager::PackageSourceClosureLimits::default(),
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
        omega_package_manager::package_compilation_inputs(&closure).map_err(|errors| {
            vec![psi_diagnostics::Diagnostic::error(format!(
                "cannot construct inspected compiler package graph: {errors:?}"
            ))]
        })?;
    let closure_subject = omega_package_manager::CanonicalSourceClosureSubject::from_resolved(
        &closure,
        omega_package_manager::CanonicalSourceClosureSubjectLimits::default(),
    )
    .map_err(|error| {
        vec![psi_diagnostics::Diagnostic::error(format!(
            "cannot project inspected package closure: {error}"
        ))]
    })?;
    let identity_roots = closure
        .custodies()
        .iter()
        .filter_map(|custody| match custody.key().source_lineage() {
            omega_package_manager::SourceLineage::ExternalLocal(lineage) => {
                Some(SourceInspectionRoot::new(
                    custody.snapshot_root(),
                    lineage.canonical_absolute_path(),
                ))
            }
            _ => None,
        })
        .collect();
    let mut snapshot = inspect_source_closure_with_packages(
        &arguments.repository_root,
        &root_snapshot.join(entry_relative),
        arguments.target_name.as_deref(),
        package_inputs,
        identity_roots,
    )?;
    snapshot.package_source_closure = Some(PackageSourceClosureCustodySnapshot {
        subject_encoding_version: omega_package_manager::SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
        subject_fingerprint: closure_subject.fingerprint().to_hex(),
        canonical_subject_bytes_hex: encode_hex(closure_subject.canonical_bytes()),
    });
    Ok(snapshot)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

struct Arguments {
    repository_root: PathBuf,
    root_path: PathBuf,
    target_name: Option<String>,
    feature_census: bool,
}

fn parse_arguments(arguments: impl Iterator<Item = OsString>) -> Option<Arguments> {
    let mut repository_root = None;
    let mut root_path = None;
    let mut target_name = None;
    let mut feature_census = false;
    let mut arguments = arguments;
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
        feature_census,
    })
}
