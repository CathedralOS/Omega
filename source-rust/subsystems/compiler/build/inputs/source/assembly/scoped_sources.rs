//! Select from one capture using the libraries supplied for Build or requested for Product.

use super::generated::append_stage_generated_sources;
use super::imports::{SourceRouting, load_source_frontiers, parse_admitted};
use super::toolchain::{append_build_prelude, seed_core_vocabulary, sources_mention_floats};
use super::{AssembledSyntax, BuildSourceSelection, BuildStageAcquisition, DeferredProductSources};
use crate::source::source_imports::{ImportQueue, SourceStorage};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use std::sync::Arc;

struct ReselectedSources {
    storage: SourceStorage,
    imports: ImportQueue,
    generated: Vec<(source_text::SourceId, build_output::PackageGeneratedSource)>,
    build_source: Option<BuildSourceSelection>,
}

impl super::ImmutableSourceParseCheckpoint {
    fn reselect(
        &self,
        inputs: &crate::source::source_imports::ProductQueryInputs,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<ReselectedSources, Vec<Diagnostic>> {
        let checkpoint = self;
        checkpoint.validate_child(inputs.original())?;
        let mut storage = checkpoint.source_storage.as_ref().clone();
        inputs.admit_source_owner(&mut storage)?;
        let loaded = storage.reselect_current_sources(catalogue)?;
        inputs.mount(&mut storage)?;
        let build_source = checkpoint
            .build_source
            .map(|original| {
                storage
                    .sources
                    .get(original.source_id())
                    .and_then(|file| storage.sources.rejoined_source_id(file))
                    .map(|source_id| original.rejoined(source_id))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "source assembly lost its selected Build declaration",
                        )]
                    })
            })
            .transpose()?;
        let parsed = parse_admitted(&mut storage, loaded.into_iter().collect(), timings)?;
        let discovered = inputs.discover(&parsed, &mut storage)?;
        storage.attach_parsed(parsed);
        let mut imports = ImportQueue::default();
        for source in storage.sources.selected_files() {
            imports.mark_loaded(&source.path, source.dependency_scope);
        }
        imports.enqueue(discovered)?;
        let generated =
            append_stage_generated_sources(&mut storage, &mut imports, inputs, timings, catalogue)?;
        Ok(ReselectedSources {
            generated,
            storage,
            imports,
            build_source,
        })
    }
}

impl DeferredProductSources {
    /// A local interpreted product can name its real declarations only after
    /// the Build returned and every direct registration has an immutable resolution.
    /// Resolve the files first so only the toolchain inputs this product uses
    /// enter its key. Issue those same occurrences under the completed owner
    /// before the declaration checker runs.
    pub fn assemble_returned_successor(
        &self,
        inputs: &crate::source_imports::ProductQueryInputs,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<
        (
            source_compiler::ProductDeclarationSource,
            Option<crate::CompletedAnonymousSource>,
        ),
        Vec<Diagnostic>,
    > {
        let projected = self.assemble_query(inputs, timings, catalogue)?;
        let checkpoint = &self.checkpoint;
        let selected = &projected.parsed().files;
        if !selected.selected_files().any(|file| {
            file.origin == source_admission::SourceOrigin::User
                && file.package_identity.is_none()
                && file.dependency_scope == source_provenance::DependencyScope::Product
                && file.package_root == checkpoint.source_storage.default_package_root()
        }) {
            return Ok((projected, None));
        }
        let mut storage = checkpoint.source_storage.as_ref().clone();
        storage.sources = selected.as_ref().clone();
        let capture = storage.retain_anonymous_owner(&[])?.ok_or_else(|| {
            vec![Diagnostic::error(
                "unowned successor declarations require their original source capture",
            )]
        })?;
        let sealed = capture.complete_product(inputs.resolved_edges()?)?;
        let mut returned = self.clone();
        let storage = Arc::make_mut(&mut returned.checkpoint.source_storage);
        storage.bind_returned_product(&sealed)?;
        let projected = returned.assemble_query(inputs, timings, catalogue)?;
        Ok((projected, Some(sealed)))
    }

    pub fn assemble_query(
        &self,
        inputs: &crate::source::source_imports::ProductQueryInputs,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<source_compiler::ProductDeclarationSource, Vec<Diagnostic>> {
        let checkpoint = &self.checkpoint;
        let ReselectedSources {
            mut storage,
            mut imports,
            build_source,
            generated: _,
        } = checkpoint.reselect(inputs, timings, catalogue)?;
        if inputs.successor_entry().is_none()
            && let Some(roots) = &checkpoint.product_roots
        {
            for root in roots.iter() {
                imports.seed(root.clone(), source_provenance::DependencyScope::Product);
            }
        }
        load_source_frontiers(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            timings,
            catalogue,
        )?;
        for path in inputs.declaration_sources(&mut storage)? {
            imports.seed(path, source_provenance::DependencyScope::Product);
        }
        load_source_frontiers(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            timings,
            catalogue,
        )?;
        let core_root = crate::source::source_imports::bundled_core_root()?;
        seed_core_vocabulary(
            &mut imports,
            &core_root,
            source_provenance::DependencyScope::Product,
        );
        for scope in [
            source_provenance::DependencyScope::Build,
            source_provenance::DependencyScope::Product,
        ] {
            if sources_mention_floats(&storage, scope) {
                imports.seed(core_root.join("float_operations.omg"), scope);
            }
        }
        load_source_frontiers(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            timings,
            catalogue,
        )?;
        let mut bindings = append_build_prelude(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            build_source,
            timings,
            catalogue,
        )?;
        bindings.extend(crate::source::source_imports::retain_import_bindings(
            &storage,
        )?);
        let root_sources = storage.root_sources();
        let build_scope_sources = storage
            .sources
            .selected_files()
            .filter(|file| file.dependency_scope == source_provenance::DependencyScope::Build)
            .map(|file| file.source_id)
            .collect();
        storage.syntax_trees.share_pages();
        let entry = build_source.ok_or_else(|| {
            vec![Diagnostic::error(
                "Product query requires a selected Build declaration",
            )]
        })?;
        Ok(source_compiler::ProductDeclarationSource::new(
            source_compiler::BuildStageSource {
                parsed: source_compiler::ParsedStageSources {
                    roots: root_sources,
                    syntax: storage.syntax_trees,
                    files: Arc::new(storage.sources),
                    imports: bindings,
                },
                build_sources: build_scope_sources,
                entry: entry.entry(),
                pending_imports: Vec::new(),
            },
        ))
    }
}

impl super::ImmutableSourceParseCheckpoint {
    pub fn assemble_supplied_build_libraries(
        &self,
        libraries: Arc<[crate::source_imports::SuppliedBuildLibrary]>,
        providers: &crate::InterpretedProviderSelections,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<AssembledSyntax, Vec<Diagnostic>> {
        let root = self.source_storage.default_package_root();
        for library in libraries.iter() {
            if library
                .dependency()
                .inputs
                .packages()
                .any(|(_, package)| package == root)
            {
                return Err(vec![Diagnostic::error(
                    "a Build library cannot supply an owner for the receiving source",
                )]);
            }
        }
        let mounted = libraries
            .iter()
            .map(|library| library.dependency().clone())
            .collect();
        let inputs = crate::source_imports::ProductQueryInputs::for_build_stage(None, mounted)
            .map_err(|why| vec![Diagnostic::error(why)])?;
        let mut assembled = self
            .assemble_build_stage(&inputs, providers, timings, catalogue)?
            .1;
        let acquisition = assembled.build_source_acquisition.as_mut().ok_or_else(|| {
            vec![Diagnostic::error(
                "a supplied Build library requires its receiving source capture",
            )]
        })?;
        acquisition.supplied_libraries = libraries;
        Ok(assembled)
    }

    fn assemble_build_stage(
        &self,
        inputs: &crate::source::source_imports::ProductQueryInputs,
        providers: &crate::InterpretedProviderSelections,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        let checkpoint = self;
        let ReselectedSources {
            mut storage,
            mut imports,
            build_source,
            generated: generated_source_custody,
        } = checkpoint.reselect(inputs, timings, catalogue)?;
        load_source_frontiers(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            timings,
            catalogue,
        )?;
        super::imports::load_provider_sources(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            providers,
            timings,
            catalogue,
        )?;
        let core_root = crate::source::source_imports::bundled_core_root()?;
        seed_core_vocabulary(
            &mut imports,
            &core_root,
            source_provenance::DependencyScope::Build,
        );
        if sources_mention_floats(&storage, source_provenance::DependencyScope::Build) {
            imports.seed(
                core_root.join("float_operations.omg"),
                source_provenance::DependencyScope::Build,
            );
        }
        load_source_frontiers(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            timings,
            catalogue,
        )?;
        let mut deferred = checkpoint.clone_shared();
        deferred.rejoin_sources(&storage.sources, inputs.original(), timings, catalogue)?;
        inputs.admit_source_owner(Arc::make_mut(&mut deferred.source_storage))?;
        let mut bindings = append_build_prelude(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            build_source,
            timings,
            catalogue,
        )?;
        bindings.extend(crate::source::source_imports::retain_import_bindings(
            &storage,
        )?);
        let root_sources = storage.root_sources();
        let source_file_count = storage.file_count();
        let pending_build_imports = crate::source::source_imports::pending_build_imports(&storage);
        let build_scope_sources = storage
            .sources
            .selected_files()
            .filter(|file| file.dependency_scope == source_provenance::DependencyScope::Build)
            .map(|file| file.source_id)
            .collect();
        let acquisition = storage.unbound_acquisition.clone();
        let capture = storage.retain_anonymous_owner(&generated_source_custody)?;
        let (sources, anonymous_owner) = match capture {
            Some(capture) => (capture.sources().clone(), Some(capture)),
            None => (Arc::new(storage.sources), None),
        };
        storage.syntax_trees.share_pages();
        Ok((
            source_file_count,
            AssembledSyntax {
                build_source_acquisition: acquisition.map(|capture| BuildStageAcquisition {
                    capture,
                    supplied_libraries: Arc::default(),
                    sources: Arc::clone(&sources),
                }),
                parsed: source_compiler::ParsedStageSources {
                    roots: root_sources,
                    syntax: storage.syntax_trees,
                    files: sources,
                    imports: bindings,
                },
                anonymous_owner,
                product_sources: Some(DeferredProductSources {
                    checkpoint: deferred,
                }),
                default_product_entry: None,
                build_source,
                generated_source_custody,
                build_scope_sources,
                pending_build_imports,
            },
        ))
    }
}
