/*
Join the activation's complete dependency-request ledger to the package
manager's acquired source graph (`BuildDependencySelection`,
`join_preacquired_sources`, `join_dependency_sources`, `validate_source_join`).

Why an alias graph alone never discharges the `Build` request ledger:
1. Every `BuildRequest::Dependency` issued during `Build` execution is assigned a
   purpose-local ordinal (`Product` or `Build`). `validate_dependency_sources`
   checks that the root package name and role match the executed
   `BuildDeclaration`, that every issued dependency request matches its exact
   `BuildDependencySelection` and `dependency_acquisitions` record in ordinal
   order, that the joined graph contains no unrequested direct dependencies, and
   that every dependency actually consumed during `Build` (`demanded_sources`)
   is present in the joined selections.
2. Before Product compilation begins, `validate_source_join` verifies that the
   `PackageCompilationInputs` passed to Product preparation carry the exact
   `PackageCompilationSourceInputs` that were joined on `ReturnedBuild` (and
   rejects unbound contexts that attempt to adopt a multi-package dependency
   graph without executed-request settlement).
*/

use super::ReturnedBuild;
use build_declarations::{BuildDeclaration, BuildRequest, DependencyPurpose};
use package_compilation::PackageCompilationInputs;
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDependencySelection {
    pub purpose: DependencyPurpose,
    pub request_ordinal: usize,
    pub package: PackageKeyIdentity,
}

impl ReturnedBuild {
    pub fn join_preacquired_sources(
        &mut self,
        inputs: &PackageCompilationInputs,
    ) -> Result<(), String> {
        if self.source_join.is_some() {
            return self.validate_source_join(Some(inputs), false);
        }
        if self.declaration.is_none() {
            return Err("unbound default Build cannot adopt a package source graph".into());
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

    pub fn join_dependency_sources(
        &mut self,
        inputs: &PackageCompilationInputs,
        selections: &[BuildDependencySelection],
    ) -> Result<(), String> {
        if self.source_join.is_some() {
            return Err("Build source requests were already joined".into());
        }
        validate_dependency_sources(
            self.declaration.as_ref(),
            self.requests.iter().map(|issued| issued.request()),
            &self.demanded_sources,
            inputs,
            selections,
        )?;
        self.source_join = Some(inputs.source_inputs());
        Ok(())
    }

    pub fn validate_source_join(
        &self,
        inputs: Option<&PackageCompilationInputs>,
        initially_unbound: bool,
    ) -> Result<(), String> {
        validate_source_join(
            self.declaration.as_ref(),
            self.source_join.as_ref(),
            inputs,
            initially_unbound,
        )
    }
}

pub(crate) fn validate_dependency_sources<'request>(
    declaration: Option<&BuildDeclaration>,
    requests: impl Iterator<Item = &'request BuildRequest>,
    demanded_sources: &[BuildDependencySelection],
    inputs: &PackageCompilationInputs,
    selections: &[BuildDependencySelection],
) -> Result<(), String> {
    let declaration = declaration.ok_or("default Build cannot acquire a new source graph")?;
    let name = match declaration {
        BuildDeclaration::Package(package) => package.name.as_str(),
        BuildDeclaration::Application(application) => application.name.as_str(),
        BuildDeclaration::Workspace(_) => {
            return Err("workspace Build has no Product source join".into());
        }
    };
    if inputs.package_name(inputs.root()) != Some(name) || inputs.root_role() != declaration.kind()
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
    for request in requests {
        let BuildRequest::Dependency { purpose, alias, .. } = request else {
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
            return Err("acquired dependency does not match its executed alias and purpose".into());
        }
        let acquired = acquired.ok_or("issued dependency has no exact acquired request custody")?;
        if acquired.request_ordinal != *ordinal
            || &acquired.request != request
            || acquired.package != selected.package
        {
            return Err("issued dependency differs from its exact acquired source request".into());
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
    for demanded in demanded_sources {
        if !selections.iter().any(|selected| {
            selected.purpose == demanded.purpose
                && selected.request_ordinal == demanded.request_ordinal
                && selected.package == demanded.package
        }) {
            return Err("joined source graph differs from a dependency used during Build".into());
        }
    }
    Ok(())
}

pub(crate) fn validate_source_join(
    declaration: Option<&BuildDeclaration>,
    source_join: Option<&std::sync::Arc<package_compilation::PackageCompilationSourceInputs>>,
    inputs: Option<&PackageCompilationInputs>,
    initially_unbound: bool,
) -> Result<(), String> {
    if let Some(joined) = source_join {
        let inputs = inputs.ok_or("joined Build sources have no completion inputs")?;
        if joined.as_ref() != inputs.source_inputs().as_ref() {
            return Err(
                "Build completion source graph differs from its executed request join".into(),
            );
        }
    } else if declaration.is_none() && inputs.is_some() {
        return Err("unbound default Build cannot adopt a package source graph".into());
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
