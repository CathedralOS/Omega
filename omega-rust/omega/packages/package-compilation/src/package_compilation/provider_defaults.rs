// Dependency-owned defaults retain their completed Build handoff and rejoin
// only after its immutable source custody and activation observation match the
// receiving package graph. Core's float realization table supplies no default
// handoff: its source capture is admitted independently by source preparation.

use super::{PackageBuildHandoff, PackageCompilationInputs};
use build_time_evaluation::provider_declarations::{ProviderSelection, RetainedProviderSelection};
use diagnostics::Diagnostic;
use source::{SourceFile, SourceResolutionStratum};
use std::path::PathBuf;
use std::sync::Arc;
use symbols::RetainedDeclaration;

pub(super) struct CompletedProviderDefaults {
    supplier: PackageBuildHandoff,
}

impl CompletedProviderDefaults {
    pub(super) fn from_package(bundle: &PackageBuildHandoff) -> Result<Option<Self>, &'static str> {
        if bundle.provider_selections().is_empty() {
            return Ok(None);
        }
        bundle
            .selection_observation()
            .ok_or("package defaults have no completed Build observation")?;
        Ok(Some(Self {
            supplier: bundle.clone(),
        }))
    }

    fn selections(&self) -> &[RetainedProviderSelection] {
        self.supplier.provider_selections()
    }

    pub(crate) fn reached_source_seeds(
        &self,
        inputs: &PackageCompilationInputs,
        loaded_sources: Option<&source::SourceMap>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        inputs.admit_provider_activation(&self.supplier)?;
        let package_sliced = loaded_sources.is_some_and(|sources| {
            sources
                .selected_files()
                .any(|file| file.package_identity == Some(self.supplier.package()))
        });
        let mut paths = Vec::new();
        for selection in self.selections() {
            let mut subject_paths = Vec::new();
            let mut selection_paths = Vec::new();
            let subject_source = self.admit_source(inputs, selection.subject.source())?;
            if subject_source.resolution_stratum == SourceResolutionStratum::Base {
                subject_paths.push(subject_source.path.clone());
                selection_paths.push(subject_source.path);
            }
            let provider_source = self.admit_source(inputs, selection.provider.source())?;
            if provider_source.resolution_stratum == SourceResolutionStratum::Base {
                selection_paths.push(provider_source.path);
            }
            for (is_subject, application) in [
                (true, &selection.subject_application),
                (false, &selection.provider_application),
            ] {
                let Some(application) = application else {
                    continue;
                };
                for span in application.declaration_spans() {
                    let file = application.sources.get(span.source_id).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "retained application has a foreign source coordinate",
                        )]
                    })?;
                    let source = self.admit_source(inputs, file)?;
                    if source.resolution_stratum == SourceResolutionStratum::Base {
                        if is_subject {
                            subject_paths.push(source.path.clone());
                        }
                        selection_paths.push(source.path);
                    }
                }
            }
            let reached = !package_sliced
                || loaded_sources.is_none_or(|sources| {
                    sources
                        .selected_files()
                        .any(|file| subject_paths.contains(&file.path))
                });
            if reached {
                paths.extend(selection_paths);
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub(super) fn rejoin(
        &self,
        source: &mut psi_compiler::SourceContinuation,
        inputs: &PackageCompilationInputs,
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        inputs.admit_provider_activation(&self.supplier)?;
        let mut selections = Vec::new();
        // One guarded span lookup serves every selection's two declarations.
        let mut candidates = symbols::SourceSpanCandidates::default();
        for selection in self.selections() {
            let mut selected = selection.clone();
            selected.subject = self.admit_declaration(inputs, &selected.subject)?;
            selected.provider = self.admit_declaration(inputs, &selected.provider)?;
            for application in [
                &mut selected.subject_application,
                &mut selected.provider_application,
            ]
            .into_iter()
            .flatten()
            {
                let needed = application
                    .declaration_spans()
                    .map(|span| span.source_id)
                    .collect::<Vec<_>>();
                let files = application
                    .sources
                    .files()
                    .map(|file| {
                        if needed.contains(&file.source_id) {
                            self.admit_source(inputs, file)
                        } else {
                            Ok(file.clone())
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                application.sources = Arc::new(source::SourceMap::from_files(files));
            }
            let subject_loaded = source.typed().symbols.source_files().any(|file| {
                file.dependency_scope == source::DependencyScope::Product
                    && source
                        .typed()
                        .symbols
                        .source_map()
                        .is_none_or(|sources| sources.source_is_selected(file.source_id))
                    && file.path == selected.subject.source().path
            });
            if !subject_loaded {
                continue;
            }
            selections.push(
                source
                    .rejoin_product_provider_selection(&selected, &mut candidates)
                    .map_err(|error| vec![error])?,
            );
        }
        Ok(selections)
    }

    fn admit_declaration(
        &self,
        inputs: &PackageCompilationInputs,
        anchor: &RetainedDeclaration,
    ) -> Result<RetainedDeclaration, Vec<Diagnostic>> {
        inputs.admit_retained_product_declaration(&self.supplier, anchor)
    }

    fn admit_source(
        &self,
        inputs: &PackageCompilationInputs,
        original: &SourceFile,
    ) -> Result<SourceFile, Vec<Diagnostic>> {
        inputs.admit_retained_product_source(&self.supplier, original)
    }
}
