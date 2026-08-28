use omega_compiler::{
    PackageSourceClosureCustodySnapshot, SourceInspectionRoot, inspect_source_closure,
    inspect_source_closure_with_packages,
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
    inspect_with_cache(arguments, local_source_cache_root())
}

fn inspect_with_cache(
    arguments: &Arguments,
    source_cache_root: PathBuf,
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
        source_cache_root,
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
    let closure_subject = omega_packages::CanonicalSourceClosureSubject::from_resolved(
        &closure,
        omega_packages::CanonicalSourceClosureSubjectLimits::default(),
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
            omega_packages::SourceLineage::ExternalLocal(lineage) => {
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
        !arguments.semantic_only,
        package_inputs,
        identity_roots,
    )?;
    snapshot.package_source_closure = Some(PackageSourceClosureCustodySnapshot {
        subject_encoding_version: omega_packages::SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-source-snapshot-root-only-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn root_only_application_still_retains_package_closure_custody() {
        let root = temp_root();
        std::fs::create_dir_all(&root).expect("create root-only application");
        std::fs::write(
            root.join("build.omg"),
            concat!(
                "machine build(builder: &mut Build) {\n",
                "    builder.application(\"closure-observer\");\n",
                "}\n",
            ),
        )
        .expect("write root-only build entry");
        std::fs::write(
            root.join("main.omg"),
            "data Main {}\nmachine Main::main(&mut self) {}\n",
        )
        .expect("write root-only source entry");
        let arguments = Arguments {
            repository_root: root.clone(),
            root_path: root.join("main.omg"),
            target_name: None,
            semantic_only: true,
            feature_census: false,
        };

        let cache = root.with_extension("cache");
        let snapshot = inspect_with_cache(&arguments, cache.clone())
            .expect("root-only package-aware source observation");

        let custody = snapshot
            .package_source_closure
            .expect("declared application must retain package source custody");
        assert_eq!(
            custody.subject_encoding_version,
            omega_packages::SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION
        );
        assert_eq!(custody.subject_fingerprint.len(), 64);
        assert!(!custody.canonical_subject_bytes_hex.is_empty());
        assert!(snapshot.sources.iter().any(|source| {
            source.package_identity.is_some()
                && source.package_relative_path.as_deref() == Some("main.omg")
        }));

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(cache);
    }
}
