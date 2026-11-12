// Source-seed discovery and declaration admission for dependency-owned Product
// provider selections (`product_selection_source_seeds`,
// `rejoin_package_provider_selections`, `admit_retained_product_declaration`).
// Authenticates each retained base or generated source file against the
// receiving compilation's immutable `SourceFileSystem` or supplying
// `PackageBuildHandoff` before rejoining declaration handles.

use super::provider_defaults::CompletedProviderDefaults;
use super::{PackageBuildHandoff, PackageCompilationInputs};
use build_declarations::DependencyPurpose;
use build_time_evaluation::provider_declarations::ProviderSelection;
use diagnostics::Diagnostic;
use source::{SourceOrigin, SourceResolutionStratum};
use std::path::PathBuf;
use symbols::RetainedDeclaration;

impl PackageCompilationInputs {
    pub(crate) fn product_selection_source_seeds(
        &self,
        loaded_sources: Option<&source::SourceMap>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let mut paths = Vec::new();
        for (scope, bundle) in self.dependency_build_handoff_instances() {
            if scope != DependencyPurpose::Product {
                continue;
            }
            self.admit_provider_activation(bundle)?;
            let package_sliced = loaded_sources.is_some_and(|sources| {
                sources
                    .selected_files()
                    .any(|file| file.package_identity == Some(bundle.package()))
            });
            for declaration in bundle.native_declarations() {
                let anchor =
                    self.admit_retained_product_declaration(bundle, &declaration.requirement)?;
                if anchor.source().resolution_stratum == SourceResolutionStratum::Base {
                    let reached = !package_sliced
                        || declaration.dependency_alias.is_some()
                        || loaded_sources.is_none_or(|sources| {
                            sources
                                .selected_files()
                                .any(|file| file.path == anchor.source().path)
                        });
                    if reached {
                        paths.push(anchor.source().path.clone());
                    }
                }
            }
            if let Some(defaults) = CompletedProviderDefaults::from_package(bundle)
                .map_err(|reason| vec![Diagnostic::error(reason)])?
            {
                paths.extend(defaults.reached_source_seeds(self, loaded_sources)?);
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
            if let Some(defaults) = CompletedProviderDefaults::from_package(bundle)
                .map_err(|reason| vec![Diagnostic::error(reason)])?
            {
                selections.extend(defaults.rejoin(source, self)?);
            }
        }
        Ok(selections)
    }

    pub(super) fn admit_provider_activation(
        &self,
        bundle: &PackageBuildHandoff,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(first) = bundle.provider_selections().first() else {
            return Ok(());
        };
        let source = first.origin.activation().source();
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let relative = source
            .path
            .strip_prefix(&source.package_root)
            .map_err(|_| {
                refuse("package provider activation is outside its original source root".into())
            })?;
        let root = self.package_root(bundle.package()).ok_or_else(|| {
            refuse("package provider activation has no admitted source supplier".into())
        })?;
        if source.package_identity != Some(bundle.package())
            || bundle.selection_observation().is_none()
        {
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
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let source = self.admit_retained_product_source(bundle, anchor.source())?;
        RetainedDeclaration::from_retained_parts(source, anchor.span(), anchor.kind())
            .map_err(|reason| refuse(reason.into()))
    }

    pub(super) fn admit_retained_product_source(
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
            let core = crate::source::source_imports::bundled_core_root()?;
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
                let owner = source.package_identity.ok_or_else(|| {
                    refuse("retained generated provider source has no supplying package".into())
                })?;
                let supplying_bundle = if owner == bundle.package() {
                    bundle
                } else {
                    self.dependency_build_handoff_instances()
                        .find(|(scope, supplied)| *scope == DependencyPurpose::Product && supplied.package() == owner)
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
