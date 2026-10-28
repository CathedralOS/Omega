//! Admit dependency-owned Product choices through their complete Build handoff.
//!
//! The private declaration was selected in the supplier's checked world. Source
//! admission authenticates that exact unit before the receiving world rejoins
//! it; no application import, private-name lookup or dependency Build execution
//! is introduced. Activation authorship stays on the original receipt.

use super::{PackageBuildHandoff, PackageCompilationInputs};
use build_declarations::DependencyPurpose;
use build_time_evaluation::provider_declarations::{ProviderPlanningContext, ProviderSelection};
use diagnostics::Diagnostic;
use source::{SourceOrigin, SourceResolutionStratum};
use std::path::PathBuf;
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
            }
            for row in bundle.native_bindings() {
                for anchor in row.anchors() {
                    let anchor = self.admit_retained_product_declaration(bundle, anchor)?;
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
                selections.push(selected.rejoin(context, &[]).map_err(|error| vec![error])?);
            }
        }
        Ok(selections)
    }

    /// Admit the supplier bytes of checked native intent before the receiving
    /// provider planner rejoins its declaration and application identities.
    pub fn retained_package_native_bindings(
        &self,
    ) -> Result<Vec<super::retained_native_bindings::RetainedNativeBinding>, Vec<Diagnostic>> {
        let mut rows = Vec::new();
        for (scope, bundle) in self.dependency_build_handoff_instances() {
            if scope != DependencyPurpose::Product {
                continue;
            }
            self.admit_provider_activation(bundle)?;
            for row in bundle.native_bindings() {
                let mut row = row.clone();
                row.requirement =
                    self.admit_retained_product_declaration(bundle, &row.requirement)?;
                if let Some(table) = &mut row.table {
                    table.table = self.admit_retained_product_declaration(bundle, &table.table)?;
                    table.field = self.admit_retained_product_declaration(bundle, &table.field)?;
                }
                // Authorship need not be a runtime Product declaration. Its
                // bytes still belong to the exact admitted source supplier.
                self.admit_retained_product_declaration(bundle, row.origin.author())?;
                if let Some(demand) = &row.same_stack_demand {
                    self.admit_retained_product_declaration(bundle, demand.origin.author())?;
                }
                rows.push(row);
            }
        }
        Ok(rows)
    }

    fn admit_provider_activation(
        &self,
        bundle: &PackageBuildHandoff,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some((origin, observed)) = bundle
            .provider_selections()
            .first()
            .map(|first| (&first.origin, bundle.selection_observation().is_some()))
            .or_else(|| {
                bundle
                    .native_bindings()
                    .first()
                    .map(|first| (&first.origin, first.observation != [0; 32]))
            })
        else {
            return Ok(());
        };
        let source = origin.activation().source();
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
        if source.package_identity != Some(bundle.package()) || !observed {
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
            SourceResolutionStratum::CurrentActivationExtension
            | SourceResolutionStratum::DependencyActivationExtension => {
                // Selection ownership and declaration ownership are distinct:
                // A may select a provider generated by its dependency B.
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
        RetainedDeclaration::from_retained_parts(source, anchor.span(), anchor.kind())
            .map_err(|reason| refuse(reason.into()))
    }
}
