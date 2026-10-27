//! Join the activation's complete request ledger to package-owned acquisitions.
//! These selections describe resolved source work, never review grants. The
//! compiler authenticates the return and compares its retained graph at completion.

use super::ReturnedBuild;
use build_declarations::{BuildDeclaration, BuildRequest, DependencyPurpose};
use package_compilation::PackageCompilationInputs;
use semantic_vocabulary::PackageKeyIdentity;

/// One package-manager selection at an issued purpose-local request ordinal.
/// Selections retain request order within each purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDependencySelection {
    pub purpose: DependencyPurpose,
    pub request_ordinal: usize,
    pub package: PackageKeyIdentity,
}

impl ReturnedBuild {
    /// Rejoin sources acquired before this bound compilation began. They must
    /// retain the exact request values as well as the alias graph; a graph alone
    /// never settles an issued request. Anonymous discovery still joins through
    /// its resolver after executing the selected sources.
    pub fn join_preacquired_sources(
        &mut self,
        inputs: &PackageCompilationInputs,
    ) -> Result<(), String> {
        if self.source_join.is_some() {
            return self.validate_source_join(Some(inputs), false);
        }
        if inputs.dependency_acquisitions().is_empty() {
            return if self
                .requests
                .iter()
                .any(|issued| matches!(issued.request(), BuildRequest::Dependency { .. }))
            {
                Err(
                    "dependency requests require coordinator settlement before Product compilation"
                        .into(),
                )
            } else {
                Ok(())
            };
        }
        let selections = inputs
            .dependency_acquisitions()
            .iter()
            .map(|acquired| {
                let BuildRequest::Dependency { purpose, .. } = acquired.request else {
                    return Err("acquired source record is not a dependency request".to_owned());
                };
                Ok(BuildDependencySelection {
                    purpose,
                    request_ordinal: acquired.request_ordinal,
                    package: acquired.package,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.join_dependency_sources(inputs, &selections)
    }

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
        let mut product_acquisitions = inputs.dependency_acquisitions().iter().filter(|acquired| {
            matches!(
                &acquired.request,
                BuildRequest::Dependency {
                    purpose: DependencyPurpose::Product,
                    ..
                }
            )
        });
        let mut build_acquisitions = inputs.dependency_acquisitions().iter().filter(|acquired| {
            matches!(
                &acquired.request,
                BuildRequest::Dependency {
                    purpose: DependencyPurpose::Build,
                    ..
                }
            )
        });
        for issued in &self.requests {
            let BuildRequest::Dependency { purpose, alias, .. } = issued.request() else {
                continue;
            };
            let (ordinal, selected, acquired) = match purpose {
                DependencyPurpose::Product => (
                    &mut product_count,
                    product_selections.next(),
                    product_acquisitions.next(),
                ),
                DependencyPurpose::Build => (
                    &mut build_count,
                    build_selections.next(),
                    build_acquisitions.next(),
                ),
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
            let acquired =
                acquired.ok_or("issued dependency has no exact acquired request custody")?;
            if acquired.request_ordinal != *ordinal
                || &acquired.request != issued.request()
                || acquired.package != selected.package
            {
                return Err(
                    "issued dependency differs from its exact acquired source request".into(),
                );
            }
            *ordinal += 1;
        }
        if selections.len() != product_count + build_count
            || inputs.dependency_acquisitions().len() != product_count + build_count
            || inputs
                .direct_dependencies(DependencyPurpose::Product)
                .count()
                != product_count
            || inputs.direct_dependencies(DependencyPurpose::Build).count() != build_count
        {
            return Err("joined source graph contains unrequested dependency selections".into());
        }
        for provider in &self.provider_sources {
            if !selections.iter().any(|selected| {
                selected.purpose == provider.purpose
                    && selected.request_ordinal == provider.request_ordinal
                    && selected.package == provider.package
            }) {
                return Err(
                    "joined source graph differs from a provider executed during Build".into(),
                );
            }
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
