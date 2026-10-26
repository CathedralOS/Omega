//! Join the activation's complete request ledger to package-owned acquisitions.
//! These selections describe resolved source work, never review grants. The
//! compiler authenticates the return and compares its retained graph at completion.

use super::ReturnedBuild;
use build_declarations::{BuildDeclaration, BuildRequest, DependencyPurpose};
use package_compilation::PackageCompilationInputs;

pub use package_compilation::BuildDependencySelection;

impl ReturnedBuild {
    /// The host resolver supplies the sources it acquired for these actual
    /// requests. This joins every ordinal and alias, then retains the exact
    /// graph; it neither acquires sources nor accepts a static declaration roster.
    pub fn join_dependency_sources(
        &mut self,
        inputs: &PackageCompilationInputs,
        selections: &[BuildDependencySelection],
    ) -> Result<(), String> {
        if self.source_join.is_some() {
            return Err("Build source requests were already joined".into());
        }
        let name = match &self.declaration {
            BuildDeclaration::Package(package) => package.name.as_str(),
            BuildDeclaration::Application(application) => application.name.as_str(),
            BuildDeclaration::Workspace(_) => {
                return Err("workspace Build has no Product source join".into());
            }
        };
        if inputs.package_name(inputs.root()) != Some(name)
            || inputs.root_role() != self.declaration.kind()
        {
            return Err("source join does not match the executed Build declaration".into());
        }
        let mut product_count = 0;
        let mut build_count = 0;
        let mut product_selections = selections
            .iter()
            .filter(|selection| selection.purpose == DependencyPurpose::Product);
        let mut build_selections = selections
            .iter()
            .filter(|selection| selection.purpose == DependencyPurpose::Build);
        for issued in &self.requests {
            let BuildRequest::Dependency { purpose, alias, .. } = issued.request() else {
                continue;
            };
            let (ordinal, selected) = match purpose {
                DependencyPurpose::Product => (&mut product_count, product_selections.next()),
                DependencyPurpose::Build => (&mut build_count, build_selections.next()),
            };
            let selected = selected.ok_or_else(|| {
                format!(
                    "Build {} dependency {} has no acquired selection",
                    purpose.name(),
                    ordinal
                )
            })?;
            if selected.request_ordinal != *ordinal {
                return Err("Build dependency selections do not retain request order".into());
            }
            let declared_name = inputs
                .package_name(selected.package)
                .ok_or("selected dependency is absent from the joined source graph")?;
            let alias = alias.as_ref().map_or(declared_name, |alias| alias.as_str());
            if inputs.dependency_target_for_purpose(inputs.root(), *purpose, alias)
                != Some(selected.package)
            {
                return Err(
                    "acquired dependency does not match its executed alias and purpose".into(),
                );
            }
            *ordinal += 1;
        }
        if selections.len() != product_count + build_count
            || inputs
                .direct_dependencies(DependencyPurpose::Product)
                .count()
                != product_count
            || inputs.direct_dependencies(DependencyPurpose::Build).count() != build_count
        {
            return Err("joined source graph contains unrequested dependency selections".into());
        }
        self.source_join = Some(inputs.source_inputs());
        Ok(())
    }

    /// Before Product preparation, reject a different graph from the one whose
    /// requests joined. Unbound contexts cannot gain undeclared dependency names.
    pub fn validate_source_join(
        &self,
        inputs: Option<&PackageCompilationInputs>,
        initially_unbound: bool,
    ) -> Result<(), String> {
        if let Some(joined) = &self.source_join {
            let inputs = inputs.ok_or("joined Build sources have no completion inputs")?;
            if joined.as_ref() != inputs.source_inputs().as_ref() {
                return Err(
                    "Build completion source graph differs from its executed request join".into(),
                );
            }
        } else if initially_unbound
            && inputs.is_some_and(|inputs| {
                inputs.packages().count() != 1
                    || DependencyPurpose::ALL
                        .into_iter()
                        .any(|purpose| inputs.direct_dependencies(purpose).next().is_some())
            })
        {
            return Err("joined package dependencies require executed-request settlement".into());
        }
        Ok(())
    }
}
