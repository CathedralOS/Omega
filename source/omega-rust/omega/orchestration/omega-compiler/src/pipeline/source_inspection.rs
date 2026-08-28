use crate::pipeline::stages::source_files_to_syntax_trees_for_engine;
use crate::pipeline::timing::CompileTimings;
use psi_diagnostics::Diagnostic;
use psi_source::SourceOrigin;
use psi_syntax_trees::SyntaxTreesSnapshot;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::path::PathBuf;

pub const SOURCE_CLOSURE_SNAPSHOT_SCHEMA: &str = "omega.source-closure-snapshot.v3";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceClosureSnapshotEntry {
    pub source_id: usize,
    pub identity: String,
    pub origin: &'static str,
    pub byte_length: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceClosureSnapshot {
    pub schema: &'static str,
    pub entry_source: String,
    pub selected_target: Option<String>,
    pub native_provider_substitution: bool,
    pub sources: Vec<SourceClosureSnapshotEntry>,
    pub syntax: SyntaxTreesSnapshot,
}

/// Map one immutable package snapshot back to the live repository location
/// whose logical identity the checkpoint records. Compilation reads only the
/// immutable root; this mapping affects diagnostic/checkpoint names, never
/// source authority or bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInspectionRoot {
    physical_root: PathBuf,
    logical_root: PathBuf,
}

impl SourceInspectionRoot {
    pub fn new(physical_root: impl Into<PathBuf>, logical_root: impl Into<PathBuf>) -> Self {
        Self {
            physical_root: physical_root.into(),
            logical_root: logical_root.into(),
        }
    }
}

impl SourceClosureSnapshot {
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn feature_census(&self) -> crate::pipeline::SourceFeatureCensus {
        crate::pipeline::census_source_closure(self)
    }
}

pub fn inspect_source_closure(
    repository_root: &Path,
    root_path: &Path,
    target_name: Option<&str>,
    native: bool,
) -> Result<SourceClosureSnapshot, Vec<Diagnostic>> {
    let repository_root = repository_root.to_owned();
    let root_path = root_path.to_owned();
    let target_name = target_name.map(str::to_owned);
    super::compiler::run_on_compile_thread(move || {
        inspect_source_closure_inner(
            &repository_root,
            &root_path,
            target_name.as_deref(),
            native,
            None,
            &[],
        )
    })
}

pub fn inspect_source_closure_with_packages(
    repository_root: &Path,
    root_path: &Path,
    target_name: Option<&str>,
    native: bool,
    packages: super::PackageCompilationInputs,
    identity_roots: Vec<SourceInspectionRoot>,
) -> Result<SourceClosureSnapshot, Vec<Diagnostic>> {
    let repository_root = repository_root.to_owned();
    let root_path = root_path.to_owned();
    let target_name = target_name.map(str::to_owned);
    super::compiler::run_on_compile_thread(move || {
        inspect_source_closure_inner(
            &repository_root,
            &root_path,
            target_name.as_deref(),
            native,
            Some(&packages),
            &identity_roots,
        )
    })
}

fn inspect_source_closure_inner(
    repository_root: &Path,
    root_path: &Path,
    target_name: Option<&str>,
    native: bool,
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
    let (_, assembled) = source_files_to_syntax_trees_for_engine(
        root_path,
        target_name,
        native,
        packages,
        &mut timings,
    )?;
    let mut sources = Vec::with_capacity(assembled.files.len());
    for parsed in assembled.files.iter() {
        let Some(source) = assembled.sources.get(parsed.source_id) else {
            return Err(vec![Diagnostic::error(format!(
                "source inspection lost source id {}",
                parsed.source_id.0
            ))]);
        };
        let identity = logical_source_identity(
            &repository_root,
            &source.path,
            source.origin,
            &identity_roots,
        )?;
        let bytes = source.source.as_bytes();
        sources.push(SourceClosureSnapshotEntry {
            source_id: source.source_id.0,
            identity,
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

    Ok(SourceClosureSnapshot {
        schema: SOURCE_CLOSURE_SNAPSHOT_SCHEMA,
        entry_source,
        selected_target: target_name.map(str::to_owned),
        native_provider_substitution: native,
        sources,
        syntax: assembled.syntax_trees.snapshot(),
    })
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
                .strip_prefix(&mapping.physical_root)
                .ok()
                .map(|relative| mapping.logical_root.join(relative))
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
            let physical_root = mapping.physical_root.canonicalize().map_err(|error| {
                vec![Diagnostic::error(format!(
                    "failed to canonicalize inspected physical root {}: {error}",
                    mapping.physical_root.display()
                ))]
            })?;
            let logical_root = mapping.logical_root.canonicalize().map_err(|error| {
                vec![Diagnostic::error(format!(
                    "failed to canonicalize inspected logical root {}: {error}",
                    mapping.logical_root.display()
                ))]
            })?;
            Ok(SourceInspectionRoot {
                physical_root,
                logical_root,
            })
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
