//! Assemble the captured source forest for the selected Build and its deferred Product.

mod build_entry;
mod checkpoint;
mod generated;
mod imports;
mod scoped_sources;
#[cfg(test)]
mod tests;
mod toolchain;

pub use build_entry::BuildSourceSelection;

use crate::PackageCompilationInputs;
use crate::source::source_imports::{
    ImportQueue, PackageImportPhase, PendingPackageImport, SourceStorage, discover_imports,
    discover_package_imports,
};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use generated::append_dependency_build_handoffs_to_storage;
use imports::{SourceRouting, load_imports, parse_admitted};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use toolchain::{append_build_prelude, seed_core_vocabulary, sources_mention_floats};

/// The root capture, selected Build libraries and source map used to check this Build.
#[derive(Clone)]
pub struct BuildStageAcquisition {
    capture: crate::CapturedLocalSource,
    supplied_libraries: Arc<[crate::source_imports::SuppliedBuildLibrary]>,
    sources: Arc<source_compiler::representations::source_inputs::SourceMap>,
}

impl BuildStageAcquisition {
    /// Caller bindings belong to the particular capture and typed source map that selected them.
    pub fn supplied_libraries_for(
        &self,
        capture: &crate::CapturedLocalSource,
        typed: &source_compiler::representations::source_state::TypedTrees,
    ) -> Option<Arc<[crate::source_imports::SuppliedBuildLibrary]>> {
        self.retains(capture, typed)
            .then(|| Arc::clone(&self.supplied_libraries))
    }

    fn retains(
        &self,
        capture: &crate::CapturedLocalSource,
        typed: &source_compiler::representations::source_state::TypedTrees,
    ) -> bool {
        &self.capture == capture
            && typed
                .symbols
                .source_map()
                .is_some_and(|selected| Arc::ptr_eq(&self.sources, &selected))
    }
}

#[derive(Clone)]
pub struct AssembledSyntax {
    pub parsed: source_compiler::ParsedStageSources,
    pub build_source_acquisition: Option<BuildStageAcquisition>,
    pub anonymous_owner: Option<crate::AnonymousSourceCapture>, // Unbound anonymous capture carried until `Product` sealing.
    pub product_sources: Option<DeferredProductSources>, // Deferred `Product` continuation held during `Build` evaluation.
    pub default_product_entry: Option<source_admission::RetainedDeclaration>,
    pub build_source: Option<BuildSourceSelection>, // Exact authored or compiler-default `Build` source selection.
    pub generated_source_custody:
        Vec<(source_text::SourceId, build_output::PackageGeneratedSource)>, // Admitted dependency-generated sources keyed by `SourceId`.
    pending_build_imports: Vec<source_compiler::PendingBuildImport>,
    pub build_scope_sources: HashSet<source_text::SourceId>, // Sources owned by the `Build` execution scope.
}

impl AssembledSyntax {
    /// The Build stage the whole-file check reads before Build runs, with the
    /// root's own files under a check-only owner. `None` when a package graph
    /// already supplies the root's owner.
    pub fn check_only_build_stage_source(
        &self,
    ) -> Result<Option<source_compiler::BuildStageSource>, Vec<Diagnostic>> {
        let Some(owner) = &self.anonymous_owner else {
            return Ok(None);
        };
        owner
            .check_only_stage(self.build_stage_source()?)
            .map(Some)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }

    pub fn build_stage_source(&self) -> Result<source_compiler::BuildStageSource, Vec<Diagnostic>> {
        let selection = self.build_source.ok_or_else(|| {
            vec![Diagnostic::error(
                "Build stage requires a selected declaration",
            )]
        })?;
        Ok(source_compiler::BuildStageSource {
            parsed: self.parsed.clone(),
            build_sources: self.build_scope_sources.clone(),
            entry: selection.entry(),
            pending_imports: self.pending_build_imports.clone(),
        })
    }
}

#[derive(record_storage::Record)]
pub struct ImmutableSourceParseCheckpoint {
    root_path: PathBuf,
    source_storage: Arc<SourceStorage>,
    build_source: Option<BuildSourceSelection>,
    package_imports: Arc<[PendingPackageImport]>,
    package_source_inputs: Option<Arc<crate::PackageCompilationSourceInputs>>,
    product_roots: Option<Arc<[PathBuf]>>,
}

#[derive(record_storage::Record)]
pub struct DeferredProductSources {
    checkpoint: ImmutableSourceParseCheckpoint,
}

impl ImmutableSourceParseCheckpoint {
    pub fn assemble(
        self,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        self.assemble_sources(package_inputs, None, timings, catalogue)
    }

    fn assemble_sources(
        mut self,
        package_inputs: Option<&PackageCompilationInputs>,
        product: Option<checkpoint::ProductSourceSelection<'_>>,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        self.validate_child(package_inputs)?;
        let deferred_product = self.product_roots.is_some().then(|| self.fork());
        let source_scope = if deferred_product.is_some() {
            source_provenance::DependencyScope::Build
        } else {
            source_provenance::DependencyScope::Product
        };
        let mut source_storage = Arc::unwrap_or_clone(self.source_storage);
        let mut imports = ImportQueue::default();
        for source in source_storage.sources.selected_files() {
            imports.mark_loaded(&source.path, source.dependency_scope);
        }
        let generated_source_custody = match package_inputs {
            Some(package_inputs) => append_dependency_build_handoffs_to_storage(
                &mut source_storage,
                &mut imports,
                package_inputs,
                source_scope,
                timings,
                catalogue,
            )?,
            None => Vec::new(),
        };
        if let Some(product) = product {
            product.seed(&mut source_storage, &mut imports, package_inputs)?;
        }
        macro_rules! load_phase {
            ($phase:expr) => {
                load_imports(
                    &mut source_storage,
                    &mut imports,
                    &self.root_path,
                    package_inputs,
                    $phase,
                    timings,
                    catalogue,
                )
            };
        }
        if let Some(package_inputs) = package_inputs {
            for request in self.package_imports.iter() {
                for resolved in request
                    .resolve_for_exact_target(package_inputs, &mut source_storage.package_roots)?
                {
                    imports.enqueue(vec![(resolved.path.clone(), request.importer_scope())])?;
                    source_storage.resolved_imports.push(resolved);
                }
            }
        }
        load_phase!(PackageImportPhase::ExactTarget(None))?;
        let seeds = if deferred_product.is_none() {
            crate::source::source_imports::entry_contract_seeds(package_inputs, &source_storage)?
        } else {
            Vec::new()
        };
        if !seeds.is_empty() {
            for seed in seeds {
                imports.seed(
                    seed.register_custody(&mut source_storage)?,
                    source_provenance::DependencyScope::Product,
                );
            }
            load_phase!(PackageImportPhase::ExactTarget(None))?;
        }
        if deferred_product.is_none()
            && let Some(inputs) = package_inputs
        {
            loop {
                let seeds = inputs.product_selection_source_seeds(Some(&source_storage.sources))?;
                let mut added = false;
                for seed in seeds {
                    let already_selected = source_storage.sources.selected_files().any(|file| {
                        file.dependency_scope == source_provenance::DependencyScope::Product
                            && file.path == seed
                    });
                    if !already_selected {
                        imports.seed(seed, source_provenance::DependencyScope::Product);
                        added = true;
                    }
                }
                if !added {
                    break;
                }
                load_phase!(PackageImportPhase::ExactTarget(None))?;
            }
        }
        let core_root = crate::source::source_imports::bundled_core_root()?;
        seed_core_vocabulary(&mut imports, &core_root, source_scope);
        if sources_mention_floats(&source_storage, source_provenance::DependencyScope::Build) {
            imports.seed(
                core_root.join("float_operations.omg"),
                source_provenance::DependencyScope::Build,
            );
        }
        if sources_mention_floats(&source_storage, source_provenance::DependencyScope::Product)
            || (deferred_product.is_none()
                && sources_mention_floats(
                    &source_storage,
                    source_provenance::DependencyScope::Build,
                ))
        {
            imports.seed(
                core_root.join("float_operations.omg"),
                source_provenance::DependencyScope::Product,
            );
        }
        load_phase!(PackageImportPhase::ExactTarget(None))?;
        if deferred_product.is_none() {
            source_storage.seal_product_owner(catalogue)?;
        }
        let loaded = source_storage.reselect_current_sources(catalogue)?;
        if let Some(inputs) = package_inputs {
            source_storage
                .sources
                .retain_product_dependency_scope(inputs.dependencies());
        }
        let parsed = parse_admitted(&mut source_storage, loaded.into_iter().collect(), timings)?;
        let discovered = match package_inputs {
            Some(inputs) => {
                discover_package_imports(
                    &parsed,
                    inputs,
                    PackageImportPhase::ExactTarget(None),
                    &mut source_storage.resolved_imports,
                    &source_storage.toolchain_contract_custody,
                    &mut source_storage.package_roots,
                    &source_storage.filesystem,
                )?
                .0
            }
            None => discover_imports(
                &parsed,
                &self.root_path,
                &source_storage.toolchain_contract_custody,
                &source_storage.filesystem,
                &mut source_storage.resolved_imports,
                &mut source_storage.package_roots,
            )?,
        };
        source_storage.attach_parsed(parsed);
        imports.enqueue(discovered)?;
        load_phase!(PackageImportPhase::ExactTarget(None))?;
        if let Some(original) = self.build_source {
            let file = source_storage
                .sources
                .get(original.source_id())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "source preparation lost its authored Build source",
                    )]
                })?;
            self.build_source = Some(
                source_storage
                    .sources
                    .rejoined_source_id(file)
                    .map(|source_id| original.rejoined(source_id))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "selected Build source has no admitted consumer correspondence",
                        )]
                    })?,
            );
        }
        let product_sources = deferred_product
            .map(|mut checkpoint| {
                checkpoint.rejoin_sources(
                    &source_storage.sources,
                    package_inputs,
                    timings,
                    catalogue,
                )?;
                Ok::<_, Vec<Diagnostic>>(DeferredProductSources { checkpoint })
            })
            .transpose()?;
        let mut source_scoped_top_level_bindings = append_build_prelude(
            &mut source_storage,
            &mut imports,
            SourceRouting::Compilation {
                root_path: &self.root_path,
                package_inputs,
                phase: PackageImportPhase::ExactTarget(None),
            },
            self.build_source,
            timings,
            catalogue,
        )?;
        source_scoped_top_level_bindings
            .extend(crate::source::source_imports::retain_import_bindings(&source_storage)?);
        let pending_build_imports =
            crate::source::source_imports::pending_build_imports(&source_storage);
        let root_sources = source_storage.root_sources();
        let source_file_count = source_storage.file_count();
        let build_scope_sources = source_storage
            .sources
            .selected_files()
            .filter(|file| file.dependency_scope == source_provenance::DependencyScope::Build)
            .map(|file| file.source_id)
            .collect();
        let capture = source_storage.retain_anonymous_owner(&generated_source_custody)?;
        let (sources, anonymous_owner) = match capture {
            Some(capture) => (capture.sources().clone(), Some(capture)),
            None => (Arc::new(source_storage.sources), None),
        };
        let default_product_entry = match (&source_storage.entry, self.build_source) {
            (Some(named), Some(build)) if build.is_default() => named.product_main(&sources)?,
            _ => None,
        };
        source_storage.syntax_trees.share_pages();
        let syntax = AssembledSyntax {
            parsed: source_compiler::ParsedStageSources {
                roots: root_sources,
                syntax: source_storage.syntax_trees,
                files: Arc::clone(&sources),
                imports: source_scoped_top_level_bindings,
            },
            build_source_acquisition: None,
            anonymous_owner,
            product_sources,
            default_product_entry,
            build_source: self.build_source,
            generated_source_custody,
            build_scope_sources,
            pending_build_imports,
        };
        Ok((source_file_count, syntax))
    }
}
