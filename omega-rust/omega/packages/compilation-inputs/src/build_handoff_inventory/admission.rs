//! Receiving source custody and declaration rejoin for the complete Build inventory.

use super::*;

impl BuildHandoffInventory {
    /// Admit the receiving source snapshot before any of its defaults join.
    pub(crate) fn admit_core(
        &self,
        current: &crate::CapturedCoreSource,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        for (_, handoff) in self.handoffs() {
            if let Some(core) = handoff.core_source() {
                if core.identity() != current.identity() || core.tree() != current.tree() {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "Core Build handoff differs from the receiving immutable Core source",
                    )]);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn extend_packages(
        &mut self,
        packages: &crate::PackageCompilationInputs,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        for (scope, handoff) in packages.dependency_build_handoff_instances() {
            self.insert(scope, handoff.clone())
                .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
        }
        Ok(())
    }

    fn admit_activation(
        &self,
        handoff: &PackageBuildHandoff,
        packages: Option<&crate::PackageCompilationInputs>,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        if let Some(core) = handoff.core_source() {
            for choice in handoff.provider_selections() {
                if handoff.selection_observation().is_none()
                    || !core.contains_source(choice.origin.activation().source())
                    || !core.contains_source(choice.origin.author().source())
                {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "Core provider selection lost its original Build activation, author or execution observation",
                    )]);
                }
            }
            Ok(())
        } else {
            packages
                .ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "package Build handoff has no admitted package source context",
                    )]
                })?
                .admit_provider_activation(handoff)
        }
    }

    fn admit_declaration(
        &self,
        handoff: &PackageBuildHandoff,
        anchor: &symbols::RetainedDeclaration,
        packages: Option<&crate::PackageCompilationInputs>,
        scope: source::DependencyScope,
    ) -> Result<symbols::RetainedDeclaration, Vec<diagnostics::Diagnostic>> {
        if anchor.source().origin == source::SourceOrigin::Toolchain {
            let core = self
                .handoffs()
                .filter_map(|(_, bundle)| bundle.core_source())
                .next()
                .ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "Toolchain provider anchor has no admitted immutable Core Build source",
                    )]
                })?;
            if !core.contains_source(anchor.source()) {
                return Err(vec![diagnostics::Diagnostic::error(
                    "Toolchain provider anchor differs from the admitted immutable Core source",
                )]);
            }
            let mut source = anchor.source().clone();
            let relative = source
                .path
                .strip_prefix(&source.package_root)
                .map_err(|_| {
                    vec![diagnostics::Diagnostic::error(
                        "Toolchain provider anchor is outside its source root",
                    )]
                })?
                .to_path_buf();
            source.path = core.root().join(relative);
            source.package_root = core.root().to_path_buf();
            source.dependency_scope = scope;
            symbols::RetainedDeclaration::from_retained_parts(source, anchor.span(), anchor.kind())
                .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])
        } else {
            packages
                .ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "provider anchor has no admitted package source context",
                    )]
                })?
                .admit_retained_product_declaration(handoff, anchor)
        }
    }

    pub fn selection_source_seeds(
        &self,
        packages: Option<&crate::PackageCompilationInputs>,
        scope: source::DependencyScope,
    ) -> Result<Vec<std::path::PathBuf>, Vec<diagnostics::Diagnostic>> {
        let mut seeds = Vec::new();
        for (consumer, handoff) in self.handoffs() {
            if !matches!(
                (consumer, scope),
                (DependencyPurpose::Build, source::DependencyScope::Build)
                    | (DependencyPurpose::Product, source::DependencyScope::Product)
            ) {
                continue;
            }
            self.admit_activation(handoff, packages)?;
            for choice in handoff.provider_selections() {
                for anchor in [&choice.subject, &choice.provider] {
                    let anchor = self.admit_declaration(handoff, anchor, packages, scope)?;
                    if anchor.source().resolution_stratum == source::SourceResolutionStratum::Base {
                        seeds.push(anchor.source().path.clone());
                    }
                }
            }
            for row in handoff.native_bindings() {
                for anchor in row.anchors() {
                    let anchor = self.admit_declaration(handoff, anchor, packages, scope)?;
                    if anchor.source().resolution_stratum == source::SourceResolutionStratum::Base {
                        seeds.push(anchor.source().path.clone());
                    }
                }
            }
        }
        seeds.sort();
        seeds.dedup();
        Ok(seeds)
    }

    pub fn rejoin_provider_selections(
        &self,
        typed: &typed_trees::TypedTrees,
        packages: Option<&crate::PackageCompilationInputs>,
        scope: source::DependencyScope,
    ) -> Result<
        Vec<build_time_evaluation::provider_declarations::ProviderSelection>,
        Vec<diagnostics::Diagnostic>,
    > {
        let context = build_time_evaluation::provider_declarations::ProviderPlanningContext::new(
            typed, scope,
        );
        let mut selected = Vec::new();
        for (consumer, handoff) in self.handoffs() {
            if !matches!(
                (consumer, scope),
                (DependencyPurpose::Build, source::DependencyScope::Build)
                    | (DependencyPurpose::Product, source::DependencyScope::Product)
            ) {
                continue;
            }
            self.admit_activation(handoff, packages)?;
            for choice in handoff.provider_selections() {
                let mut choice = choice.clone();
                choice.subject =
                    self.admit_declaration(handoff, &choice.subject, packages, scope)?;
                choice.provider =
                    self.admit_declaration(handoff, &choice.provider, packages, scope)?;
                selected.push(choice.rejoin(context, &[]).map_err(|error| vec![error])?);
            }
        }
        Ok(selected)
    }
}
