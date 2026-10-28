//! Admit dependency-owned Product choices through their complete Build handoff.
//!
//! The private declaration was selected in the supplier's checked world. Source
//! admission authenticates that exact unit before the receiving world rejoins
//! it; no application import, private-name lookup or dependency Build execution
//! is introduced. Activation authorship stays on the original receipt.

use super::{PackageCompilationInputs, PackageGeneratedSourceBundle};
use build_declarations::DependencyPurpose;
use build_time_evaluation::provider_declarations::{ProviderPlanningContext, ProviderSelection};
use diagnostics::Diagnostic;
use source::{SourceOrigin, SourceResolutionStratum};
use std::path::PathBuf;
use symbols::RetainedDeclaration;

impl PackageCompilationInputs {
    pub(crate) fn provider_selection_source_seeds(&self) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let mut paths = Vec::new();
        for (scope, bundle) in self.dependency_generated_source_instances() {
            if scope != DependencyPurpose::Product {
                continue;
            }
            for selection in bundle.provider_selections() {
                for anchor in [&selection.subject, &selection.provider] {
                    let anchor = self.admit_provider_anchor(bundle, anchor)?;
                    if anchor.source().resolution_stratum == SourceResolutionStratum::Base {
                        paths.push(anchor.source().path.clone());
                    }
                }
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    /// Reconstruct default choices on this Product's exact declaration handles.
    /// This neither grants authority nor installs Build-host service backing.
    pub fn rejoin_package_provider_selections(
        &self,
        typed: &typed_trees::TypedTrees,
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
        let mut selections = Vec::new();
        for (scope, bundle) in self.dependency_generated_source_instances() {
            if scope != DependencyPurpose::Product {
                continue;
            }
            for selection in bundle.provider_selections() {
                let mut selected = selection.clone();
                selected.subject = self.admit_provider_anchor(bundle, &selected.subject)?;
                selected.provider = self.admit_provider_anchor(bundle, &selected.provider)?;
                // Duplicate/default arbitration belongs to provider settlement,
                // where authoritative root overrides have their proper tier.
                selections.push(selected.rejoin(context, &[]).map_err(|error| vec![error])?);
            }
        }
        Ok(selections)
    }

    fn admit_provider_anchor(
        &self,
        bundle: &PackageGeneratedSourceBundle,
        anchor: &RetainedDeclaration,
    ) -> Result<RetainedDeclaration, Vec<Diagnostic>> {
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let mut source = anchor.source().clone();
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
            SourceResolutionStratum::CurrentActivationExtension => {
                let generated = bundle.sources().iter().find(|generated| {
                    let logical = source.package_root.join(".omega/generated")
                        .join(String::from_utf8_lossy(generated.relative_path()).as_ref());
                    logical == source.path
                }).ok_or_else(|| refuse("retained generated provider declaration has no exact Build source handoff".into()))?;
                if generated.bytes() != source.source.as_bytes() {
                    return Err(refuse("retained generated provider declaration differs from its Build source handoff".into()));
                }
                source.resolution_stratum = SourceResolutionStratum::DependencyActivationExtension;
            }
            SourceResolutionStratum::DependencyActivationExtension => {
                return Err(refuse("a package cannot re-export inherited generated provider declarations as its own source".into()));
            }
        }
        RetainedDeclaration::from_retained_parts(source, anchor.span(), anchor.kind())
            .map_err(|reason| refuse(reason.into()))
    }
}
