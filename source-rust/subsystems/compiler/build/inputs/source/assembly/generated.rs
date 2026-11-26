//! Attach the generated source of each admitted dependency under its recorded custody.

use crate::PackageCompilationInputs;
use crate::source::source_imports::{
    GeneratedSourceExtension, ImportQueue, SourceStorage, discover_imports_with_packages,
};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use source_admission::{SourceContents, SourceOrigin, SourceResolutionStratum};
use source_compiler::SourceInput;
use std::sync::Arc;

impl GeneratedSourceExtension {
    pub fn prepare_source_request(
        self,
        base_sources: &Arc<source_compiler::representations::source_inputs::SourceMap>,
        inputs: &PackageCompilationInputs,
        timings: &mut CompileTimings,
    ) -> Result<source_compiler::SourceExtensionRequest, Vec<Diagnostic>> {
        let (sources, custody) = self.into_source_inputs(base_sources)?;
        let first = custody
            .first()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "generated-source handoff has no selected source",
                )]
            })?
            .0;
        let mut generated_sources = Vec::with_capacity(custody.len());
        for (source_id, _) in &custody {
            let source = sources.get(*source_id).ok_or_else(|| {
                vec![Diagnostic::error(
                    "admitted generated source has no retained source instance",
                )]
            })?;
            generated_sources.push(source_admission::LoadedSource {
                source_id: *source_id,
                path: source.path.clone(),
                source: source.source.clone(),
                origin: Some(source.origin),
                scope: source.dependency_scope,
            });
        }
        let mut syntax = source_compiler::SourceSyntax::new(first);
        let parsed =
            SourceInput(generated_sources.into_iter().collect()).append(&mut syntax, timings)?;
        let top_level_bindings =
            crate::source::source_imports::generated_file_imports(&parsed, &sources, inputs)?;
        Ok(source_compiler::SourceExtensionRequest::new(
            syntax,
            sources,
            top_level_bindings,
        ))
    }
}

pub(super) fn append_dependency_build_handoffs_to_storage(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    package_inputs: &PackageCompilationInputs,
    source_scope: source_provenance::DependencyScope,
    timings: &mut CompileTimings,
    catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
) -> Result<Vec<(source_text::SourceId, build_output::PackageGeneratedSource)>, Vec<Diagnostic>> {
    let entries = crate::source::source_imports::admit_dependency_build_handoffs(
        package_inputs,
        source_scope,
    )?;

    for entry in &entries {
        imports.mark_loaded(&entry.logical_path, entry.scope);
    }

    let mut retained = Vec::with_capacity(entries.len());
    for entry in entries {
        let text = entry.source_text()?;
        let entry_base = entry.authored_base(package_inputs, &mut source_storage.package_roots)?;
        let crate::source::source_imports::DependencyGeneratedSource {
            logical_path,
            source,
            package,
            scope,
        } = entry;
        let admitted = source_storage.admit_source(
            SourceContents {
                path: logical_path,
                source: Arc::from(text),
                scope,
            },
            Some(SourceOrigin::User),
            SourceResolutionStratum::DependencyActivationExtension,
            catalogue,
        )?;
        let source_id = admitted.source_id;
        let parsed = SourceInput([admitted].into_iter().collect()).append(
            &mut source_storage.syntax_trees,
            timings,
        )?;
        let discovered = discover_imports_with_packages(
            &parsed,
            package_inputs,
            Some(package),
            source_storage,
        )?;
        imports.enqueue(discovered)?;
        imports.enqueue(entry_base)?;
        source_storage.attach_parsed(parsed);
        retained.push((source_id, source.clone()));
    }
    Ok(retained)
}

/// Admits, into the Build scope of a stage, the source each prepared build
/// dependency's own Build generated. The generated units are part of the
/// package the stage may call, in the stratum a dependency's generated
/// source always has: visible to other packages, and not to the dependency's
/// own authored source. The package's authored units are enqueued with them,
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
                crate::source::source_imports::dependency_generated_source_logical_path(
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
