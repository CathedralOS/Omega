//! Admit the frozen files before resolving any imports in the receiving compilation.

use crate::PackageCompilationInputs;
use crate::source::source_imports::{
    ImportQueue, PackageImportPhase, SourceStorage, discover_package_imports,
};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use source_admission::{SourceContents, SourceOrigin, SourceResolutionStratum};
use source_compiler::SourceInput;
use std::sync::Arc;

pub(super) fn admit_generated_files(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    package_inputs: &PackageCompilationInputs,
    own_files: &[build_output::PackageGeneratedSource],
    source_scope: source_provenance::DependencyScope,
    timings: &mut CompileTimings,
    catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
) -> Result<Vec<(source_text::SourceId, build_output::PackageGeneratedSource)>, Vec<Diagnostic>> {
    let entries = crate::source::source_imports::collect_generated_files(
        package_inputs,
        own_files,
        source_scope,
    )?;

    for entry in &entries {
        imports.mark_loaded(&entry.logical_path, entry.scope);
    }

    let mut retained = Vec::with_capacity(entries.len());
    let mut files = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.stratum == SourceResolutionStratum::DependencyActivationExtension {
            imports.enqueue(entry.authored_base(package_inputs, &mut source_storage.package_roots)?)?;
        }
        let source = Arc::from(entry.source_text()?);
        let admitted = source_storage.admit_source(
            SourceContents { path: entry.logical_path, source, scope: entry.scope },
            Some(SourceOrigin::User),
            entry.stratum,
            catalogue,
        )?;
        retained.push((admitted.source_id, entry.source.clone()));
        files.push(admitted);
    }
    if !files.is_empty() {
        let parsed = SourceInput(files.into_iter().collect())
            .append(&mut source_storage.syntax_trees, timings)?;
        let (discovered, _) = discover_package_imports(
            &parsed,
            package_inputs,
            PackageImportPhase::ExactTarget,
            source_storage,
        )?;
        imports.enqueue(discovered)?;
        source_storage.attach_parsed(parsed);
    }
    Ok(retained)
}

/// Admits, into the Build scope of a stage, the source each prepared build
/// dependency's own Build generated. The generated units are part of the
/// package the stage may call, in the stratum a dependency's generated
/// source always has, with the same logical-file visibility as authored units. The package's authored units are enqueued with them,
/// because generated code selects its package's authored declarations.
pub(super) fn append_stage_generated_sources(
    storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    inputs: &crate::source::source_imports::ProductQueryInputs,
    timings: &mut CompileTimings,
    catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
) -> Result<Vec<(source_text::SourceId, build_output::PackageGeneratedSource)>, Vec<Diagnostic>> {
    use crate::source::source_imports::LogicalSourceDiscovery as _;
    let mut retained = Vec::new();
    // Two aliases may name one package. Its generated source is admitted once.
    let mut admitted = Vec::new();
    for dependency in inputs.build_dependencies() {
        let Some(handoff) = &dependency.generated else {
            continue;
        };
        let package = dependency.inputs.root();
        if handoff.sources().is_empty() || admitted.contains(&package) {
            continue;
        }
        admitted.push(package);
        let package_root = dependency
            .inputs
            .package_root(package)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "prepared build dependency lost its source root",
                )]
            })?
            .to_path_buf();
        for generated in handoff.sources() {
            let logical_path =
                crate::source::source_imports::generated_file_path(
                    &package_root,
                    generated,
                )?;
            if storage.filesystem.exists(&logical_path) {
                return Err(vec![Diagnostic::error(format!(
                    "generated dependency source logical path `{}` collides with another source",
                    logical_path.display(),
                ))]);
            }
            let text = std::str::from_utf8(generated.bytes()).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "included generated source `{}` is not UTF-8 Omega source",
                    logical_path.display(),
                ))]
            })?;
            imports.mark_loaded(&logical_path, source_provenance::DependencyScope::Build);
            let unit = storage.admit_source(
                SourceContents {
                    path: logical_path,
                    source: Arc::from(text),
                    scope: source_provenance::DependencyScope::Build,
                },
                Some(SourceOrigin::User),
                SourceResolutionStratum::DependencyActivationExtension,
                catalogue,
            )?;
            let source_id = unit.source_id;
            let parsed = SourceInput([unit].into_iter().collect())
                .append(&mut storage.syntax_trees, timings)?;
            let discovered = inputs.discover(&parsed, storage)?;
            imports.enqueue(discovered)?;
            storage.attach_parsed(parsed);
            retained.push((source_id, generated.clone()));
        }
        let excluded = dependency
            .inputs
            .packages()
            .filter(|(owner, root)| *owner != package && root.starts_with(&package_root))
            .map(|(_, root)| root.to_path_buf())
            .collect();
        let authored = storage.package_roots.select_owned(
            &package_root,
            &[],
            &storage.filesystem,
            excluded,
        )?;
        imports.enqueue(
            authored
                .into_iter()
                .map(|path| (path, source_provenance::DependencyScope::Build))
                .collect(),
        )?;
    }
    Ok(retained)
}
