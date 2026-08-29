use crate::pipeline::source_assembly::source_files_to_syntax_trees_for_engine;
use crate::pipeline::timing::CompileTimings;
pub use omega_source_profile::{
    PackageSourceClosureCustodySnapshot, SOURCE_CLOSURE_SNAPSHOT_SCHEMA, SourceClosureSnapshot,
    SourceClosureSnapshotEntry, SourceClosureSnapshotFingerprint, SourceInspectionRoot,
};
use psi_diagnostics::Diagnostic;
use psi_source::SourceOrigin;
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn inspect_source_closure(
    repository_root: &Path,
    root_path: &Path,
    target_name: Option<&str>,
) -> Result<SourceClosureSnapshot, Vec<Diagnostic>> {
    let repository_root = repository_root.to_owned();
    let root_path = root_path.to_owned();
    let target_name = target_name.map(str::to_owned);
    crate::compiler::execution::run_on_compile_thread(move || {
        inspect_source_closure_inner(
            &repository_root,
            &root_path,
            target_name.as_deref(),
            None,
            &[],
        )
    })
}

pub fn inspect_source_closure_with_packages(
    repository_root: &Path,
    root_path: &Path,
    target_name: Option<&str>,
    packages: super::PackageCompilationInputs,
    identity_roots: Vec<SourceInspectionRoot>,
) -> Result<SourceClosureSnapshot, Vec<Diagnostic>> {
    let repository_root = repository_root.to_owned();
    let root_path = root_path.to_owned();
    let target_name = target_name.map(str::to_owned);
    crate::compiler::execution::run_on_compile_thread(move || {
        inspect_source_closure_inner(
            &repository_root,
            &root_path,
            target_name.as_deref(),
            Some(&packages),
            &identity_roots,
        )
    })
}

fn inspect_source_closure_inner(
    repository_root: &Path,
    root_path: &Path,
    target_name: Option<&str>,
    packages: Option<&super::PackageCompilationInputs>,
    identity_roots: &[SourceInspectionRoot],
) -> Result<SourceClosureSnapshot, Vec<Diagnostic>> {
    let repository_root = repository_root.canonicalize().map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to canonicalize source-inspection repository root {}: {error}",
            repository_root.display()
        ))]
    })?;
    let identity_roots = canonical_identity_roots(identity_roots)?;
    let entry_source = logical_source_identity(
        &repository_root,
        root_path,
        SourceOrigin::User,
        &identity_roots,
    )?;
    let mut timings = CompileTimings::default();
    let (_, assembled) =
        source_files_to_syntax_trees_for_engine(root_path, target_name, packages, &mut timings)?;
    let mut sources = Vec::with_capacity(assembled.files.len());
    for parsed in assembled.files.iter() {
        let Some(source) = assembled.sources.get(parsed.source_id) else {
            return Err(vec![Diagnostic::error(format!(
                "source inspection lost source id {}",
                parsed.source_id.0
            ))]);
        };
        let bytes = source.source.as_bytes();
        let package_identity = source
            .package_identity
            .map(|identity| encode_hex(&identity.digest()));
        let package_relative_path = match source.package_identity {
            Some(_) => Some(
                source
                    .path
                    .strip_prefix(&source.package_root)
                    .ok()
                    .and_then(canonical_relative_path)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "inspected package source {} has no canonical path beneath {}",
                            source.path.display(),
                            source.package_root.display()
                        ))]
                    })?,
            ),
            None => None,
        };
        let identity = match (&package_identity, &package_relative_path) {
            (Some(package), Some(relative)) => format!("package:{package}/{relative}"),
            _ => logical_source_identity(
                &repository_root,
                &source.path,
                source.origin,
                &identity_roots,
            )?,
        };
        sources.push(SourceClosureSnapshotEntry {
            source_id: source.source_id.0,
            identity,
            package_identity,
            package_relative_path,
            origin: match source.origin {
                SourceOrigin::User => "repository",
                SourceOrigin::Toolchain if source.path.to_string_lossy().starts_with('<') => {
                    "virtual"
                }
                SourceOrigin::Toolchain => "toolchain",
            },
            byte_length: bytes.len(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
        });
    }
    sources.sort_by(|left, right| left.identity.cmp(&right.identity));
    if let Some(duplicate) = sources
        .windows(2)
        .find(|pair| pair[0].identity == pair[1].identity)
    {
        return Err(vec![Diagnostic::error(format!(
            "source inspection produced duplicate canonical identity `{}`",
            duplicate[0].identity
        ))]);
    }

    Ok(SourceClosureSnapshot {
        schema: SOURCE_CLOSURE_SNAPSHOT_SCHEMA,
        entry_source,
        package_source_closure: None,
        selected_target: target_name.map(str::to_owned),
        sources,
        syntax: assembled.syntax_trees.snapshot(),
    })
}

fn canonical_relative_path(path: &Path) -> Option<String> {
    let mut components = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(component) = component else {
            return None;
        };
        components.push(component.to_str()?.to_owned());
    }
    (!components.is_empty()).then(|| components.join("/"))
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

fn logical_source_identity(
    repository_root: &Path,
    path: &Path,
    origin: SourceOrigin,
    identity_roots: &[SourceInspectionRoot],
) -> Result<String, Vec<Diagnostic>> {
    if origin == SourceOrigin::Toolchain && path.to_string_lossy().starts_with('<') {
        return Ok(path.to_string_lossy().into_owned());
    }
    let canonical = path.canonicalize().map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to canonicalize inspected source {}: {error}",
            path.display()
        ))]
    })?;
    let logical = identity_roots
        .iter()
        .find_map(|mapping| {
            canonical
                .strip_prefix(mapping.physical_root())
                .ok()
                .map(|relative| mapping.logical_root().join(relative))
        })
        .unwrap_or(canonical);
    let relative = logical.strip_prefix(repository_root).map_err(|_| {
        vec![Diagnostic::error(format!(
            "inspected source {} has no repository identity under {}",
            logical.display(),
            repository_root.display()
        ))]
    })?;
    if relative.as_os_str().is_empty() {
        return Err(vec![Diagnostic::error(
            "source identity cannot equal the repository root",
        )]);
    }
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn canonical_identity_roots(
    identity_roots: &[SourceInspectionRoot],
) -> Result<Vec<SourceInspectionRoot>, Vec<Diagnostic>> {
    identity_roots
        .iter()
        .map(|mapping| {
            let physical_root = mapping.physical_root().canonicalize().map_err(|error| {
                vec![Diagnostic::error(format!(
                    "failed to canonicalize inspected physical root {}: {error}",
                    mapping.physical_root().display()
                ))]
            })?;
            let logical_root = mapping.logical_root().canonicalize().map_err(|error| {
                vec![Diagnostic::error(format!(
                    "failed to canonicalize inspected logical root {}: {error}",
                    mapping.logical_root().display()
                ))]
            })?;
            Ok(SourceInspectionRoot::new(physical_root, logical_root))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_source_identity_is_stable() {
        assert_eq!(
            logical_source_identity(
                Path::new("/irrelevant"),
                Path::new("<build-prelude>"),
                SourceOrigin::Toolchain,
                &[],
            )
            .expect("virtual identity"),
            "<build-prelude>"
        );
    }
}
