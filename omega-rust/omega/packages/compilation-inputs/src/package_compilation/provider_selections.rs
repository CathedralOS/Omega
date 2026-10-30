//! Admit dependency-owned Product choices through their complete Build handoff.
//!
//! The private declaration was selected in the supplier's checked world. Source
//! admission authenticates that exact unit before the receiving world rejoins
//! it; no application import, private-name lookup or dependency Build execution
//! is introduced. Activation authorship stays on the original receipt.

use super::{PackageBuildHandoff, PackageCompilationInputs};
use build_declarations::DependencyPurpose;
use build_time_evaluation::provider_declarations::ProviderSelection;
use diagnostics::Diagnostic;
use source::{SourceOrigin, SourceResolutionStratum};
use symbols::RetainedDeclaration;

impl PackageCompilationInputs {
    pub(crate) fn product_selection_source_seeds(&self) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let mut paths = Vec::new();
        for (scope, bundle) in self.dependency_build_handoff_instances() {
            if scope != DependencyPurpose::Product {
                continue;
            }
            self.admit_provider_activation(bundle)?;
            for selection in bundle.provider_selections() {
                for anchor in [&selection.subject, &selection.provider] {
                    let anchor = self.admit_retained_product_declaration(bundle, anchor)?;
                    if anchor.source().resolution_stratum == SourceResolutionStratum::Base {
                        paths.push(anchor.source().path.clone());
                    }
                }
                for application in [
                    &selection.subject_application,
                    &selection.provider_application,
                ]
                .into_iter()
                .flatten()
                {
                    for span in application.declaration_spans() {
                        let file = application.sources.get(span.source_id).ok_or_else(|| {
                            vec![Diagnostic::error(
                                "retained application has a foreign source coordinate",
                            )]
                        })?;
                        let source = self.admit_retained_product_source(bundle, file)?;
                        if source.resolution_stratum == SourceResolutionStratum::Base {
                            paths.push(source.path);
                        }
                    }
                }
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
    pub fn rejoin_package_provider_selections(
        &self,
        source: &mut psi_compiler::SourceContinuation,
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        let mut selections = Vec::new();
        for (scope, bundle) in self.dependency_build_handoff_instances() {
            if scope != DependencyPurpose::Product {
                continue;
            }
            self.admit_provider_activation(bundle)?;
            for selection in bundle.provider_selections() {
                let mut selected = selection.clone();
                selected.subject =
                    self.admit_retained_product_declaration(bundle, &selected.subject)?;
                selected.provider =
                    self.admit_retained_product_declaration(bundle, &selected.provider)?;
                // Duplicate/default arbitration belongs to provider settlement,
                // where authoritative root overrides have their proper tier.
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
                                self.admit_retained_product_source(bundle, file)
                            } else {
                                Ok(file.clone())
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    application.sources = std::sync::Arc::new(source::SourceMap::from_files(files));
                }
                selections.push(
                    source
                        .rejoin_product_provider_selection(&selected)
                        .map_err(|error| vec![error])?,
                );
            }
        }
        Ok(selections)
    }

    pub(crate) fn admit_provider_activation(
        &self,
        bundle: &PackageBuildHandoff,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some((origin, observed)) = bundle
            .provider_selections()
            .first()
            .map(|first| (&first.origin, bundle.selection_observation().is_some()))
        else {
            return Ok(());
        };
        let package = bundle.package().ok_or_else(|| {
            vec![Diagnostic::error(
                "Core Build activation requires captured Toolchain source custody",
            )]
        })?;
        let source = origin.activation().source();
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let relative = source
            .path
            .strip_prefix(&source.package_root)
            .map_err(|_| {
                refuse("package provider activation is outside its original source root".into())
            })?;
        let root = self.package_root(package).ok_or_else(|| {
            refuse("package provider activation has no admitted source supplier".into())
        })?;
        if source.package_identity != Some(package) || !observed {
            return Err(refuse(
                "package provider activation lost its original package or evaluation observation"
                    .into(),
            ));
        }
        let current = self
            .source_filesystem()
            .read_to_string(&root.join(relative))
            .map_err(|error| {
                refuse(format!(
                    "cannot admit original package provider activation: {error}"
                ))
            })?;
        if current != source.source {
            return Err(refuse(
                "package provider activation differs from its immutable source supplier".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn admit_retained_product_declaration(
        &self,
        bundle: &PackageBuildHandoff,
        anchor: &RetainedDeclaration,
    ) -> Result<RetainedDeclaration, Vec<Diagnostic>> {
        let source = self.admit_retained_product_source(bundle, anchor.source())?;
        RetainedDeclaration::from_retained_parts(source, anchor.span(), anchor.kind())
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }

    fn admit_retained_product_source(
        &self,
        bundle: &PackageBuildHandoff,
        original: &source::SourceFile,
    ) -> Result<source::SourceFile, Vec<Diagnostic>> {
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let mut source = original.clone();
        let relative = source
            .path
            .strip_prefix(&source.package_root)
            .map_err(|_| refuse("retained provider source is outside its supplier root".into()))?
            .to_path_buf();
        if source.origin == SourceOrigin::Toolchain {
            let core = crate::source_imports::bundled_core_root()?;
            if !source.path.starts_with(core) {
                return Err(refuse(
                    "retained provider anchor is outside the closed core toolchain".into(),
                ));
            }
        } else {
            let owner = source.package_identity.ok_or_else(|| {
                refuse("retained provider declaration has no immutable supplier package".into())
            })?;
            let root = self.package_root(owner).ok_or_else(|| refuse(
                "retained provider declaration belongs to a package outside the admitted closure".into(),
            ))?;
            source.path = root.join(&relative);
            source.package_root = root.to_path_buf();
        }
        match source.resolution_stratum {
            SourceResolutionStratum::Base => {
                let current = self
                    .source_filesystem()
                    .read_to_string(&source.path)
                    .map_err(|error| {
                        refuse(format!(
                            "cannot admit retained provider source {}: {error}",
                            source.path.display()
                        ))
                    })?;
                if current != source.source {
                    return Err(refuse(
                        "retained provider source differs from immutable supplier custody".into(),
                    ));
                }
            }
            SourceResolutionStratum::CurrentActivationExtension
            | SourceResolutionStratum::DependencyActivationExtension => {
                // Selection ownership and declaration ownership are distinct:
                // A may select a provider generated by its dependency B.
                let owner = source.package_identity.ok_or_else(|| {
                    refuse("retained generated provider source has no supplying package".into())
                })?;
                let supplying_bundle = if Some(owner) == bundle.package() {
                    bundle
                } else {
                    self.dependency_build_handoff_instances()
                        .find(|(scope, supplied)| *scope == DependencyPurpose::Product && supplied.package() == Some(owner))
                        .map(|(_, supplied)| supplied)
                        .ok_or_else(|| refuse("retained generated provider source has no handoff for its supplying package and purpose".into()))?
                };
                let relative = source.path.strip_prefix(source.package_root.join(".omega/generated"))
                    .map_err(|_| refuse("retained generated provider source is outside its exact generated namespace".into()))?
                    .to_path_buf();
                let generated = supplying_bundle.generated_source(&[relative])
                    .map_err(|reason| refuse(reason.into()))?
                    .ok_or_else(|| refuse("retained generated provider declaration has no exact Build source handoff".into()))?;
                if generated.bytes() != source.source.as_bytes() {
                    return Err(refuse("retained generated provider declaration differs from its Build source handoff".into()));
                }
                source.resolution_stratum = SourceResolutionStratum::DependencyActivationExtension;
            }
        }
        Ok(source)
    }
}
