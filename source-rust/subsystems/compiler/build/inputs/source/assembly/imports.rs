//! Load the selected import frontier and retain the parser-issued units.

use crate::PackageCompilationInputs;
use crate::source::source_imports::{
    ImportQueue, PackageImportPhase, PendingPackageImport, SourceStorage, discover_imports,
    discover_package_imports, validate_package_source_frontier,
};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use source_admission::SourceResolutionStratum;
use source_compiler::SourceInput;
use std::path::PathBuf;

pub(super) fn load_imports(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    package_inputs: Option<&PackageCompilationInputs>,
    phase: PackageImportPhase,
    timings: &mut CompileTimings,
    catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
) -> Result<Vec<PendingPackageImport>, Vec<Diagnostic>> {
    load_source_frontiers(
        source_storage,
        imports,
        SourceRouting::Compilation {
            package_inputs,
            phase,
        },
        timings,
        catalogue,
    )
}

#[derive(Clone, Copy)]
pub(super) enum SourceRouting<'inputs> {
    Compilation {
        package_inputs: Option<&'inputs PackageCompilationInputs>,
        phase: PackageImportPhase,
    },
    Query(&'inputs crate::source::source_imports::ProductQueryInputs),
}

/// Provider declarations can be in files outside the ordinary import closure.
/// Load each needed file from the same immutable mounts that supplied this Build.
pub(super) fn load_provider_sources(
    storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    routing: SourceRouting<'_>,
    providers: &crate::InterpretedProviderSelections,
    timings: &mut CompileTimings,
    catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
) -> Result<(), Vec<Diagnostic>> {
    if providers.is_empty() {
        return Ok(());
    }
    let mut queued = std::collections::HashSet::new();
    loop {
        let mut added = false;
        for path in providers.required_source_paths(&storage.sources)? {
            if queued.insert(path.clone()) {
                imports.seed(path, source_provenance::DependencyScope::Build);
                added = true;
            }
        }
        if !added {
            return Ok(());
        }
        let unresolved = load_source_frontiers(storage, imports, routing, timings, catalogue)?;
        if let Some(first) = unresolved.first() {
            return Err(vec![Diagnostic::error(format!(
                "interpreter provider import has no supplied source: {first:?}"
            ))]);
        }
    }
}

pub(super) fn load_source_frontiers(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    routing: SourceRouting<'_>,
    timings: &mut CompileTimings,
    catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
) -> Result<Vec<PendingPackageImport>, Vec<Diagnostic>> {
    let mut retained_requests = Vec::new();
    while let Some(frontier) = imports.take_frontier() {
        let frontier = match &routing {
            SourceRouting::Compilation {
                package_inputs: Some(package_inputs),
                ..
            } => validate_package_source_frontier(frontier, package_inputs, source_storage)?,
            SourceRouting::Query(inputs) => inputs.validate_frontier(frontier, source_storage)?,
            _ => frontier,
        };
        for (path, scope) in &frontier {
            imports.enqueue(source_storage.operator_family_sources(path, *scope)?)?;
        }
        let loaded = crate::source::source_loading::load_source_files(
            frontier,
            &source_storage.filesystem,
            timings,
        )?;
        let admitted = loaded
            .into_iter()
            .map(|contents| {
                source_storage.admit_source(
                    contents,
                    None,
                    SourceResolutionStratum::Base,
                    catalogue,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if source_storage.source_roots.is_empty()
            && let Some(first) = admitted.first()
        {
            source_storage
                .syntax_trees
                .set_primary_source(first.source_id);
        }
        let parsed = parse_admitted(source_storage, admitted.into_iter().collect(), timings)?;
        let discovered = routing.discover(&parsed, source_storage, &mut retained_requests)?;
        imports.enqueue(discovered)?;
        source_storage.attach_parsed(parsed);
    }
    Ok(retained_requests)
}

impl SourceRouting<'_> {
    /// The sources the `use` items of one parsed batch select, under the
    /// import rules of this routing. Every source enters a compilation
    /// through here, the toolchain's Build prelude included.
    pub(super) fn discover(
        &self,
        parsed: &source_compiler::ParsedSourceBatch,
        source_storage: &mut SourceStorage,
        retained_requests: &mut Vec<PendingPackageImport>,
    ) -> Result<Vec<(PathBuf, source_provenance::DependencyScope)>, Vec<Diagnostic>> {
        match self {
            SourceRouting::Compilation {
                package_inputs: Some(package_inputs),
                phase,
                ..
            } => {
                let (discovered, mut requests) = discover_package_imports(
                    parsed,
                    package_inputs,
                    *phase,
                    &mut source_storage.resolved_imports,
                    &source_storage.toolchain_contract_custody,
                    &mut source_storage.package_roots,
                    &source_storage.filesystem,
                )?;
                retained_requests.append(&mut requests);
                Ok(discovered)
            }
            SourceRouting::Compilation {
                package_inputs: None,
                ..
            } => discover_imports(parsed, source_storage),
            SourceRouting::Query(inputs) => inputs.discover(parsed, source_storage),
        }
    }
}

/// Parses admitted sources into the storage's forest. A file discovery
/// already parsed (`retained_unit`) adopts that parse when its bytes are
/// unchanged, so selecting a logical module does not parse it a second time.
pub(super) fn parse_admitted(
    storage: &mut SourceStorage,
    admitted: source_admission::LoadedSources,
    timings: &mut CompileTimings,
) -> Result<source_compiler::ParsedSourceBatch, Vec<Diagnostic>> {
    let owners = &storage.package_roots;
    SourceInput(admitted).append_reusing(
        &mut storage.syntax_trees,
        timings,
        |source| {
            storage
                .entry
                .as_ref()
                .filter(|entry| entry.path == source.path)
                .map(|entry| entry.syntax.as_ref())
                .or_else(|| crate::source::source_imports::retained_unit(owners, &source.path))
        },
    )
}
