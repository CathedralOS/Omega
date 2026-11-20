// Dependency-owned defaults retain their completed Build handoff and rejoin
// only after its immutable source custody and activation observation match the
// receiving package graph. Core's float realization table supplies no default
// handoff: its source capture is admitted independently by source preparation.

use super::{PackageBuildHandoff, PackageCompilationInputs};
use diagnostics::Diagnostic;
use source_admission::RetainedDeclaration;
use source_admission::SourceFile;
use source_admission::SourceResolutionStratum;
use source_compiler::product::provider_declarations::{
    RetainedOperatorFamily, RetainedProviderSelection, RetainedProviderSubject,
};
use std::path::PathBuf;

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
        loaded_sources: Option<&source_compiler::source_inputs::SourceMap>,
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
            for declaration in selection.subject.declarations() {
                let source = self.admit_source(inputs, declaration.source())?;
                if source.resolution_stratum == SourceResolutionStratum::Base {
                    subject_paths.push(source.path.clone());
                    selection_paths.push(source.path);
                }
            }
            let provider_source = self.admit_source(inputs, selection.provider.source())?;
            if provider_source.resolution_stratum == SourceResolutionStratum::Base {
                selection_paths.push(provider_source.path);
            }
            for (is_subject, application) in [
                (true, selection.subject.application()),
                (false, selection.provider_application.as_ref()),
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

    pub(super) fn admit(
        &self,
        inputs: &PackageCompilationInputs,
    ) -> Result<Vec<RetainedProviderSelection>, Vec<Diagnostic>> {
        inputs.admit_provider_activation(&self.supplier)?;
        let mut selections = Vec::new();
        for selection in self.selections() {
            let mut selected = selection.clone();
            match &mut selected.subject {
                RetainedProviderSubject::Declaration {
                    declaration,
                    application,
                    ..
                } => {
                    *declaration = self.admit_declaration(inputs, declaration)?;
                    if let Some(application) = application {
                        *application = inputs
                            .admit_retained_product_application(&self.supplier, application)?;
                    }
                }
                RetainedProviderSubject::OperatorFamily(family) => {
                    let mut members = family.coordinates().to_vec();
                    for member in &mut members {
                        member.declaration = self.admit_declaration(inputs, &member.declaration)?;
                    }
                    *family = RetainedOperatorFamily::new(family.canonical_path.clone(), members)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                }
            }
            selected.provider = self.admit_declaration(inputs, &selected.provider)?;
            if let Some(application) = &mut selected.provider_application {
                *application =
                    inputs.admit_retained_product_application(&self.supplier, application)?;
            }
            selections.push(selected);
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
