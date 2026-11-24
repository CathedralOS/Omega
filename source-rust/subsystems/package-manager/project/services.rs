//! Acquire the exact dependency occurrences requested by a suspended Build.
use super::{PreparedRequest, SettledSource, SourceHost};
use crate::PackageCommandError;
use crate::resolution::source::{AcquiredBuildSource, AcquiredSourceSelection};
use build_declarations::{BuildRequest, DependencyPurpose};
use build_execution::host::bindings::BuildProductQuery;
use build_execution::host::requests::BuildDependencyPreparation;
use build_execution::{CompileFailure, TerminalBuildActivation};
use diagnostics::Diagnostic;
use package_compilation::source_imports::ProductQueryDependency;
use package_compilation::{PackageBuildHandoff, PackageCompilationInputs};
use project_build::PlanningServices;
use std::collections::BTreeMap;

pub(super) struct SourceServices<'a, 's, 'p> {
    pub host: &'a mut SourceHost<'s, 'p>,
    pub acquired: &'a AcquiredBuildSource,
    pub requests: &'a mut BTreeMap<(DependencyPurpose, usize), PreparedRequest>,
}

fn refused(reason: impl Into<String>) -> CompileFailure {
    vec![Diagnostic::error(reason)].into()
}

fn failure(error: PackageCommandError) -> CompileFailure {
    error.compilation_failure().cloned().unwrap_or_else(|| {
        refused(error.to_string())
            .with_source_completion_accounting(error.source_completion_accounting().clone())
    })
}

fn aliases(
    activation: &TerminalBuildActivation,
    purpose: DependencyPurpose,
) -> impl Iterator<Item = Option<&str>> {
    activation
        .requests()
        .iter()
        .filter_map(move |issued| match issued.request() {
            BuildRequest::Dependency {
                purpose: registered,
                alias,
                ..
            } if *registered == purpose => Some(alias.as_ref().map(|alias| alias.as_str())),
            _ => None,
        })
}

impl SourceServices<'_, '_, '_> {
    /// The selected Build runs once. Its settled source and deferred Product are available
    /// both to this activation's suspension and to the subsequent final graph join.
    fn dependency(
        &mut self,
        activation: &TerminalBuildActivation,
        purpose: DependencyPurpose,
        ordinal: usize,
    ) -> Result<usize, CompileFailure> {
        if let Some(request) = self.requests.get(&(purpose, ordinal)) {
            return Ok(request.settled);
        }
        let selected = self
            .host
            .acquisitions
            .acquire_issued(self.acquired, activation.requests(), purpose, ordinal)
            .map_err(refused)?;
        let child = match selected {
            AcquiredSourceSelection::Source(child) => child,
            AcquiredSourceSelection::Workspace(workspace) => {
                self.host.select_workspace(workspace).map_err(failure)?
            }
        };
        let index = self.host.settle_dependency(&child)?;
        self.requests.insert(
            (purpose, ordinal),
            PreparedRequest {
                settled: index,
                acquisition: child,
            },
        );
        Ok(index)
    }
}

impl SourceHost<'_, '_> {
    fn library_input(
        &self,
        settled: &SettledSource,
        purpose: DependencyPurpose,
        alias: String,
    ) -> Result<ProductQueryDependency, CompileFailure> {
        let child_build_source = settled.build.evaluated().base_source_consumption_commitment;
        if purpose == DependencyPurpose::Product {
            // Its Build has returned. The Product's consumer will choose its deployment.
            return Ok(ProductQueryDependency {
                alias,
                child_build_source,
                inputs: settled.build.inputs().clone(),
                generated: None,
            });
        }
        let product = self.handoffs().dependency_product(
            &settled.build,
            purpose,
            self.profile,
            &mut DependencyProducts::default(),
        )?;
        Ok(ProductQueryDependency {
            alias,
            child_build_source,
            inputs: product
                .package_inputs()
                .ok_or_else(|| refused("prepared dependency lost its package source"))?
                .clone(),
            generated: Some(product.build_handoff(self.profile)?),
        })
    }

    pub(super) fn supplied_libraries(
        &mut self,
        receiver: &AcquiredBuildSource,
        libraries: &[super::BuildLibraryRequest],
    ) -> Result<Vec<package_compilation::source_imports::SuppliedBuildLibrary>, PackageCommandError>
    {
        use package_compilation::source_imports::SuppliedBuildLibrary;
        let mut aliases = std::collections::BTreeSet::new();
        for library in libraries {
            if !aliases.insert(&library.alias) {
                return Err(format!(
                    "duplicate supplied Build library alias `{}`",
                    library.alias.as_str()
                )
                .into());
            }
        }
        let requests: Vec<_> = libraries
            .iter()
            .map(|library| BuildRequest::Dependency {
                purpose: DependencyPurpose::Build,
                alias: Some(library.alias.clone()),
                source: library.source.clone(),
            })
            .collect();
        let mut supplied = Vec::with_capacity(libraries.len());
        for (ordinal, library) in libraries.iter().enumerate() {
            let chosen = self.acquisitions.acquire_requests(
                receiver,
                requests.iter(),
                DependencyPurpose::Build,
                ordinal,
            )?;
            let child = match chosen {
                AcquiredSourceSelection::Source(child) => child,
                AcquiredSourceSelection::Workspace(workspace) => {
                    self.select_workspace(workspace)?
                }
            };
            let index = self.settle_dependency(&child)?;
            let input = self.library_input(
                &self.settled[index],
                DependencyPurpose::Build,
                library.alias.as_str().to_owned(),
            )?;
            supplied.push(SuppliedBuildLibrary::new(library.source.clone(), input)?);
        }
        Ok(supplied)
    }

    fn settle_dependency(&mut self, child: &AcquiredBuildSource) -> Result<usize, CompileFailure> {
        self
            .acquisitions
            .admit_build_descent(&self.active, child)
            .map_err(refused)?;
        if let Some(i) = self
            .settled
            .iter()
            .position(|settled| settled.source.same_source(child))
        {
            child
                .verify_reused_capture(&self.settled[i].source)
                .map_err(refused)?;
            return Ok(i);
        }
        let stage = self.member_or_execute(child).map_err(failure)?.stage;
        let declared = child.declare_build(stage.returned()).map_err(refused)?;
        declared.require_dependency().map_err(refused)?;
        let resolved = self.join(child.clone(), stage).map_err(failure)?;
        let settled = resolved.root.settle_dependency(resolved.inputs)?;
        self
            .session
            .record_source_completion(settled.evaluated().source_completion_accounting.clone());
        let i = self.settled.len();
        self.settled.push(SettledSource {
            source: child.clone(),
            declaration: declared,
            build: settled,
            graph: resolved.sources,
        });
        Ok(i)
    }
}

impl PlanningServices for SourceServices<'_, '_, '_> {
    fn declared_source(
        &mut self,
        activation: &TerminalBuildActivation,
    ) -> Result<Option<PackageCompilationInputs>, CompileFailure> {
        self.acquired
            .declared_source_inputs(activation)
            .map(Some)
            .map_err(refused)
    }

    fn prepare_dependency(
        &mut self,
        activation: &TerminalBuildActivation,
        request: &BuildDependencyPreparation,
    ) -> Result<ProductQueryDependency, CompileFailure> {
        let i = self.dependency(activation, request.purpose, request.request_ordinal)?;
        let settled = &self.host.settled[i];
        let alias = aliases(activation, request.purpose)
            .nth(request.request_ordinal)
            .ok_or_else(|| refused("Build preparation names no issued dependency"))?
            .unwrap_or_else(|| settled.declaration.package_name())
            .to_owned();
        self.host.library_input(settled, request.purpose, alias)
    }

    fn query_dependencies(
        &mut self,
        activation: &TerminalBuildActivation,
        _query: &BuildProductQuery,
    ) -> Result<Vec<ProductQueryDependency>, CompileFailure> {
        let purpose = DependencyPurpose::Product;
        aliases(activation, purpose)
            .enumerate()
            .map(|(ordinal, alias)| {
                let i = self.dependency(activation, purpose, ordinal)?;
                let settled = &self.host.settled[i];
                Ok(ProductQueryDependency {
                    alias: alias
                        .unwrap_or_else(|| settled.declaration.package_name())
                        .to_owned(),
                    inputs: settled.build.inputs().clone(),
                    child_build_source: settled
                        .build
                        .evaluated()
                        .base_source_consumption_commitment,
                    generated: None,
                })
            })
            .collect()
    }

    fn provider_product(
        &mut self,
        handoff: &PackageBuildHandoff,
    ) -> Result<project_build::PlannedProduct, CompileFailure> {
        let settled = self
            .host
            .settled
            .iter()
            .find(|settled| settled.declaration.identity() == handoff.package())
            .ok_or_else(|| refused("Build provider source has no settled library Product"))?;
        self.host.handoffs().dependency_product(
            &settled.build,
            handoff.purpose(),
            handoff.target(),
            &mut DependencyProducts::default(),
        )
    }
}

pub(super) struct HandoffSources<'a> {
    pub accepted: Option<crate::operations::AcceptedDeployment<'a>>,
    pub profile: target::TargetProfile,
    pub settled: &'a [super::SettledSource],
}

impl SourceHost<'_, '_> {
    fn handoffs(&self) -> HandoffSources<'_> {
        HandoffSources {
            accepted: None,
            profile: self.profile,
            settled: &self.settled,
        }
    }
}

/// The completed Build handoffs for one dependency traversal. A supplier Product stays
/// with the exact occurrence that produced its handoff.
#[derive(Default)]
pub(super) struct DependencyProducts {
    by_package: BTreeMap<semantic_vocabulary::PackageKeyIdentity, Vec<CompletedDependency>>,
    independent_target: Option<target::TargetProfile>,
}

struct CompletedDependency {
    handoff: PackageBuildHandoff,
    supplier: Option<project_build::PlannedProduct>,
}

impl DependencyProducts {
    pub(super) fn into_suppliers(
        self,
    ) -> BTreeMap<semantic_vocabulary::PackageKeyIdentity, project_build::PlannedProduct> {
        self.by_package
            .into_values()
            .flatten()
            .filter_map(|row| row.supplier.map(|product| (row.handoff.package(), product)))
            .collect()
    }
}

impl HandoffSources<'_> {
    /// A provider cut can come from the Build of any dependency fused into this
    /// Product. Only a Product occurrence can ask us to retain native suppliers.
    pub(super) fn deployment_products(
        &self,
        root: &project_build::SettledDependency,
        target: target::TargetProfile,
    ) -> DependencyProducts {
        use semantic_vocabulary::source_selections::CompositionMode;
        let occurrences = root.inputs().required_dependency_source_instances();
        let mut builds = std::iter::once(root).chain(self.settled.iter().filter_map(|source| {
            occurrences
                .contains(&(source.declaration.identity(), DependencyPurpose::Product))
                .then_some(&source.build)
        }));
        let independent = builds.any(|build| {
            build
                .evaluated()
                .configuration
                .product_selection_requests
                .iter()
                .any(|request| {
                    request.composition == Some(CompositionMode::Independent)
                        && request.target.is_none_or(|selected| {
                            selected.canonical_name() == target.target_name()
                        })
                })
        });
        DependencyProducts {
            independent_target: independent.then_some(target),
            ..Default::default()
        }
    }

    /// Recursively form each package/purpose/target handoff once from that dependency's
    /// original Build return and the handoffs of its own dependencies.
    pub(super) fn dependency_product(
        &self,
        settled: &project_build::SettledDependency,
        purpose: DependencyPurpose,
        target: target::TargetProfile,
        products: &mut DependencyProducts,
    ) -> Result<project_build::PlannedProduct, CompileFailure> {
        let inputs = settled
            .inputs()
            .clone()
            .with_compilation_purpose(purpose)
            .map_err(|errors| {
                refused(format!(
                    "cannot select dependency compilation purpose: {errors:?}"
                ))
            })?;
        let inputs = match self.accepted {
            Some(policy) => policy
                .bind_compilation_inputs(inputs, target, self.profile)
                .map_err(refused)?,
            None => inputs,
        };
        let mut bundles = Vec::new();
        for (package, purpose) in inputs.required_dependency_source_instances() {
            let deployment = match purpose {
                DependencyPurpose::Product => target,
                DependencyPurpose::Build => self.profile,
            };
            if let Some(row) = products.by_package.get(&package).and_then(|rows| {
                rows.iter().find(|row| {
                    row.handoff.purpose() == purpose && row.handoff.target() == deployment
                })
            }) {
                bundles.push(row.handoff.clone());
                continue;
            }
            let child = self
                .settled
                .iter()
                .find(|child| child.declaration.identity() == package)
                .ok_or_else(|| refused("dependency graph has no corresponding settled Build"))?;
            let product = self.dependency_product(&child.build, purpose, deployment, products)?;
            let bundle = product.build_handoff(deployment)?;
            let supplier = (purpose == DependencyPurpose::Product
                && products.independent_target == Some(deployment))
            .then_some(product);
            products
                .by_package
                .entry(package)
                .or_default()
                .push(CompletedDependency {
                    handoff: bundle.clone(),
                    supplier,
                });
            bundles.push(bundle);
        }
        let inputs = inputs
            .with_complete_dependency_build_handoffs(bundles)
            .map_err(|errors| {
                refused(format!(
                    "cannot admit dependency Build handoffs: {errors:?}"
                ))
            })?;
        inputs.validate_execution_context(Some(target), Some(self.profile))?;
        settled.open_product(inputs)
    }
}
