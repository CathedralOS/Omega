//! The package manager joins the sources supplied before Build entry and the dependencies the
//! guest registered while running. Each source request must match its exact acquisition,
//! purpose-local ordinal and alias. Product compilation consumes the graph that passed this join.

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

/// Live backing explicitly installed on one executed dependency registration.
/// Source reuse and program delegation are independent parts of the same edge.
#[derive(Clone, Default)]
pub struct DependencyBacking {
    pub frozen: Option<crate::host::requests::FrozenBuildBinding>,
    pub programs: Vec<package_compilation::DelegatedProgram>,
}

impl ReturnedBuild {
    fn supplied_libraries(&self) -> &[package_compilation::source_imports::SuppliedBuildLibrary] {
        self.original_sources
            .as_ref()
            .map_or(&[], |sources| sources.libraries.as_ref())
    }

    /// A predecessor supplies each ordinary dependency in registration order. The next
    /// activation imports it in Build scope under the name established by its own handoff.
    pub(super) fn validate_successor_libraries(
        &self,
        libraries: &[package_compilation::source_imports::SuppliedBuildLibrary],
    ) -> Result<(), String> {
        let mut supplied = libraries.iter();
        for (ordinal, (source, alias, max_reach)) in self
            .requests
            .iter()
            .filter_map(|issued| match issued.request() {
                BuildRequest::Dependency {
                    purpose: DependencyPurpose::Product,
                    source,
                    alias,
                    max_reach,
                } => Some((source, alias.as_ref(), max_reach)),
                _ => None,
            })
            .enumerate()
        {
            let library = supplied
                .next()
                .ok_or("successor is missing a registered library")?;
            let dependency = library.dependency();
            let package = dependency.inputs.root();
            let expected_alias = alias
                .map(|name| name.as_str())
                .or_else(|| dependency.inputs.package_name(package));
            if !matches!(library.request(), BuildRequest::Dependency {
                purpose: DependencyPurpose::Build, source: received, ..
            } if received == source)
                || library.max_reach() != max_reach
                || expected_alias != Some(dependency.alias.as_str())
                || self.demanded_sources.iter().any(|demand| {
                    demand.purpose.is_product()
                        && demand.request_ordinal == ordinal
                        && demand.package != package
                })
            {
                return Err(
                    "successor library differs from the executed dependency registration".into(),
                );
            }
        }
        if supplied.next().is_some() {
            return Err("successor received an unregistered library".into());
        }
        Ok(())
    }

    /// Only the source coordinator combines these two sets of inputs. The guest's
    /// request log remains the record of just the operations it performed.
    /// The frozen sources of issued forwarding requests and of the incoming Build
    /// edges appended by source_requests, in that same purpose-local order.
    pub fn dependency_backings(
        &self,
    ) -> std::collections::BTreeMap<(DependencyPurpose, usize), DependencyBacking> {
        let mut backings = std::collections::BTreeMap::new();
        let mut next = [0usize; 2];
        for issued in &self.requests {
            if let BuildRequest::Dependency { purpose, .. } = issued.request() {
                let ordinal = &mut next[usize::from(purpose.canonical_tag())];
                if issued.forwarded().is_some() || !issued.delegated_programs().is_empty() {
                    backings.insert(
                        (*purpose, *ordinal),
                        DependencyBacking {
                            frozen: issued.forwarded().cloned(),
                            programs: issued.delegated_programs().to_vec(),
                        },
                    );
                }
                *ordinal += 1;
            }
        }
        if let Some(sources) = &self.original_sources {
            let first = next[usize::from(DependencyPurpose::Build.canonical_tag())];
            for index in 0..sources.libraries.len() {
                backings.insert(
                    (DependencyPurpose::Build, first + index),
                    DependencyBacking {
                        frozen: Some(crate::host::requests::FrozenBuildBinding::new(
                            std::sync::Arc::clone(&sources.libraries),
                            index,
                        )),
                        programs: Vec::new(),
                    },
                );
            }
        }
        backings
    }

    pub fn source_requests(&self) -> impl Iterator<Item = &BuildRequest> {
        self.requests.iter().map(|issued| issued.request()).chain(
            self.supplied_libraries()
                .iter()
                .map(|library| library.request()),
        )
    }

    pub fn join_preacquired_sources(
        &mut self,
        inputs: &PackageCompilationInputs,
    ) -> Result<(), String> {
        if self.source_join.is_some() {
            return self.validate_source_join(Some(inputs), false);
        }
        if self.declaration().is_none() {
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
                self.join_dependency_sources(inputs, &[])
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
        for library in self.supplied_libraries() {
            library.validate_join(inputs)?;
        }
        for issued in &self.requests {
            let Some(binding) = issued.forwarded() else {
                continue;
            };
            let BuildRequest::Dependency { max_reach, .. } = issued.request() else {
                return Err("forwarded package binding has no dependency request".into());
            };
            if !max_reach.is_subset_of(binding.library().max_reach()) {
                return Err("forwarded dependency exceeds its incoming max_reach".into());
            }
            binding
                .library()
                .validate_join_for(inputs, DependencyPurpose::Product)?;
        }
        validate_dependency_sources(
            self.declaration(),
            self.source_requests(),
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
        // A workspace publishes a member roster. Its incoming libraries were
        // acquired before entry; there is no named Product graph to join them to.
        let workspace_roster = matches!(self.declaration(), Some(BuildDeclaration::Workspace(_)))
            && inputs.is_none()
            && !self
                .requests
                .iter()
                .any(|issued| matches!(issued.request(), BuildRequest::Dependency { .. }));
        if !workspace_roster && !self.supplied_libraries().is_empty() && self.source_join.is_none()
        {
            return Err(
                "Build completion requires the source join for its supplied libraries".into(),
            );
        }
        validate_source_join(
            self.declaration(),
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
        let BuildRequest::Dependency {
            purpose,
            alias,
            max_reach,
            ..
        } = request
        else {
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
        if inputs.dependency_max_reach(inputs.root(), *purpose, alias) != Some(max_reach) {
            return Err("dependency alias changes its executed max_reach".into());
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
