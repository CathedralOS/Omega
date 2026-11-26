//! Acquire the exact dependency occurrences requested by a suspended Build.
use super::{PreparedRequest, SettledSource, SourceHost};
use crate::PackageCommandError;
use crate::resolution::source::{AcquiredBuildSource, AcquiredSourceSelection};
use build_declarations::{BuildRequest, DependencyPurpose};
use build_execution::host::bindings::BuildProductQuery;
use build_execution::host::requests::{
    BuildDependencyPreparation, BuildValidationRequest, IssuedBuildRequest,
};
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

fn registered_uses(
    activation: &TerminalBuildActivation,
    purpose: DependencyPurpose,
) -> impl Iterator<
    Item = (
        Option<&str>,
        &semantic_vocabulary::dependency_reach::DependencyReach,
    ),
> {
    activation
        .requests()
        .iter()
        .filter_map(move |issued| match issued.request() {
            BuildRequest::Dependency {
                purpose: registered,
                alias,
                max_reach,
                ..
            } if *registered == purpose => {
                Some((alias.as_ref().map(|alias| alias.as_str()), max_reach))
            }
            _ => None,
        })
}

impl SourceServices<'_, '_, '_> {
    /// The selected Build runs once. Its settled source and deferred Product are available
    /// both to this activation's suspension and to the subsequent final graph join.
    pub(super) fn dependency(
        &mut self,
        issued: &[IssuedBuildRequest],
        purpose: DependencyPurpose,
        ordinal: usize,
    ) -> Result<usize, CompileFailure> {
        if let Some(request) = self.requests.get(&(purpose, ordinal)) {
            return Ok(request.settled);
        }
        let issued_request = issued.iter().filter(|issued| matches!(issued.request(), BuildRequest::Dependency {purpose: registered, ..} if *registered == purpose)).nth(ordinal);
        let programs = issued_request
            .map(IssuedBuildRequest::delegated_programs)
            .unwrap_or_default();
        let frozen = issued_request
            .and_then(IssuedBuildRequest::forwarded)
            .map(|binding| {
                self.host
                    .frozen_library(binding.library())
                    .map(|index| self.host.settled[index].source.clone())
            })
            .transpose()?;
        let selected = self
            .host
            .acquisitions
            .acquire_requests_or_reuse(
                self.acquired,
                issued.iter().map(IssuedBuildRequest::request),
                purpose,
                ordinal,
                frozen,
            )
            .map_err(refused)?;
        let child = match selected {
            AcquiredSourceSelection::Source(child) => child,
            AcquiredSourceSelection::Workspace(workspace) => {
                self.host.select_workspace(workspace).map_err(failure)?
            }
        };
        let index = self.host.settle_dependency(&child, purpose, programs)?;
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
    pub(super) fn frozen_library(
        &self,
        library: &package_compilation::source_imports::SuppliedBuildLibrary,
    ) -> Result<usize, CompileFailure> {
        self.settled.iter().position(|settled|
            settled.build.evaluated().base_source_consumption_commitment == library.dependency().child_build_source
            && library.matches_settled_source(settled.build.inputs())
        ).ok_or_else(|| refused("incoming package binding has no matching settled Build and frozen dependency closure"))
    }

    pub(super) fn library_input(
        &self,
        settled: &SettledSource,
        purpose: DependencyPurpose,
        alias: String,
        max_reach: semantic_vocabulary::dependency_reach::DependencyReach,
    ) -> Result<ProductQueryDependency, CompileFailure> {
        let child_build_source = settled.build.evaluated().base_source_consumption_commitment;
        if purpose == DependencyPurpose::Product {
            // Its Build has returned. The Product's consumer will choose its deployment.
            return Ok(ProductQueryDependency {
                max_reach,
                alias,
                child_build_source,
                inputs: settled.build.inputs().clone(),
                generated: None,
            });
        }
        let (product, handoff) = self.handoffs().dependency_handoff(
            &settled.build,
            purpose,
            self.profile,
            &mut DependencyProducts::default(),
        )?;
        Ok(ProductQueryDependency {
            max_reach,
            alias,
            child_build_source,
            inputs: product
                .package_inputs()
                .ok_or_else(|| refused("prepared dependency lost its package source"))?
                .clone(),
            generated: Some(handoff),
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
                max_reach: library.max_reach.clone(),
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
            let index = self.settle_dependency(&child, DependencyPurpose::Build, &[])?;
            let input = self.library_input(
                &self.settled[index],
                DependencyPurpose::Build,
                library.alias.as_str().to_owned(),
                library.max_reach.clone(),
            )?;
            supplied.push(SuppliedBuildLibrary::new(library.source.clone(), input)?);
        }
        Ok(supplied)
    }

    fn settle_dependency(
        &mut self,
        child: &AcquiredBuildSource,
        purpose: DependencyPurpose,
        programs: &[package_compilation::DelegatedProgram],
    ) -> Result<usize, CompileFailure> {
        self.acquisitions
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
            super::require_programs(&self.settled[i].required_programs, programs)
                .map_err(refused)?;
            return Ok(i);
        }
        let stage = self
            .member_or_execute(child, purpose, programs)
            .map_err(failure)?
            .stage;
        let required_programs = stage.used_programs();
        let declared = child.declare_build(stage.returned()).map_err(refused)?;
        declared.require_dependency().map_err(refused)?;
        let resolved = self.join(child.clone(), stage).map_err(failure)?;
        let settled = resolved.root.settle_dependency(resolved.inputs)?;
        self.session
            .record_source_completion(settled.evaluated().source_completion_accounting.clone());
        let i = self.settled.len();
        self.settled.push(SettledSource {
            source: child.clone(),
            required_programs,
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

    fn validation_subject(
        &mut self,
        issued: &[IssuedBuildRequest],
        request: &BuildValidationRequest,
    ) -> Result<build_execution::DependencyValidationSubject, CompileFailure> {
        let index = match request {
            BuildValidationRequest::Registered { purpose, ordinal } => {
                self.dependency(issued, *purpose, *ordinal)?
            }
            BuildValidationRequest::Incoming(binding) => {
                self.host.frozen_library(binding.library())?
            }
        };
        let build = &self.host.settled[index].build;
        Ok(build_execution::DependencyValidationSubject {
            purpose: request.purpose(),
            inputs: build.inputs().clone(),
            configuration: std::sync::Arc::clone(&build.evaluated().configuration),
        })
    }

    fn prepare_dependency(
        &mut self,
        activation: &TerminalBuildActivation,
        request: &BuildDependencyPreparation,
    ) -> Result<ProductQueryDependency, CompileFailure> {
        let i = self.dependency(
            activation.requests(),
            request.purpose,
            request.request_ordinal,
        )?;
        let settled = &self.host.settled[i];
        let (alias, max_reach) = registered_uses(activation, request.purpose)
            .nth(request.request_ordinal)
            .ok_or_else(|| refused("Build preparation names no issued dependency"))?;
        let alias = alias
            .unwrap_or_else(|| settled.declaration.package_name())
            .to_owned();
        self.host
            .library_input(settled, request.purpose, alias, max_reach.clone())
    }

    fn query_dependencies(
        &mut self,
        activation: &TerminalBuildActivation,
        _query: &BuildProductQuery,
    ) -> Result<Vec<ProductQueryDependency>, CompileFailure> {
        let purpose = DependencyPurpose::Product;
        registered_uses(activation, purpose)
            .enumerate()
            .map(|(ordinal, (alias, max_reach))| {
                let i = self.dependency(activation.requests(), purpose, ordinal)?;
                let settled = &self.host.settled[i];
                Ok(ProductQueryDependency {
                    max_reach: max_reach.clone(),
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
        let products = self.host.handoffs();
        for settled in self
            .host
            .settled
            .iter()
            .filter(|settled| settled.declaration.identity() == handoff.package())
        {
            let (product, formed) = products.dependency_handoff(
                &settled.build,
                handoff.purpose(),
                handoff.target(),
                &mut DependencyProducts::default(),
            )?;
            if formed == *handoff {
                return Ok(product);
            }
        }
        Err(refused(
            "Build provider source has no settled Product matching its library handoff",
        ))
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
    /// The measured Build work of every dependency Product opened for this traversal.
    timings: Option<artifacts::compile_timings::CompileTimings>,
    /// Products opened in this traversal, in opening order, whose Build
    /// rechecks may still be running.
    pending: Vec<PendingDependency>,
}

struct CompletedDependency {
    handoff: PackageBuildHandoff,
    supplier: Option<project_build::PlannedProduct>,
}

struct PendingDependency {
    product: project_build::PendingProduct,
    /// The row that keeps this Product as a supplier, once its handoff formed.
    supplier: Option<(semantic_vocabulary::PackageKeyIdentity, usize)>,
}

/// The Build rechecks of a Product and of the dependency Products it was
/// opened from, still running while that Product is checked. [`Self::settle`]
/// waits for them; the caller reports nothing about the Product before it does.
#[derive(Default)]
#[must_use = "a Build recheck refuses the Product that it or its consumer opened"]
pub struct DependencyRechecks {
    /// In opening order, so the first refusal is the one a sequential
    /// traversal would report: each dependency's, then the Product's own.
    /// No dependency among them supplies a component.
    pending: Vec<project_build::OwedBuildRecheck>,
}

impl DependencyRechecks {
    /// The recheck a root Product without dependencies still owes, if any.
    pub(super) fn own(recheck: Option<project_build::OwedBuildRecheck>) -> Self {
        Self {
            pending: recheck.into_iter().collect(),
        }
    }

    /// Wait for every recheck in opening order and return the Build work
    /// the rechecks measured, or the first refusal.
    pub fn settle(self) -> Result<artifacts::compile_timings::CompileTimings, CompileFailure> {
        project_build::with_build_rechecks(|| {
            let mut timings = artifacts::compile_timings::CompileTimings::default();
            for pending in self.pending {
                let measured = pending.settle()?;
                if measured.is_enabled() {
                    if !timings.is_enabled() {
                        timings = artifacts::compile_timings::CompileTimings::enabled();
                    }
                    timings.extend(&measured);
                }
            }
            Ok(timings)
        })
    }
}

impl DependencyProducts {
    /// Settle every Product this traversal opened, in opening order, so the
    /// first refused recheck is the one a sequential traversal would report.
    fn settle(&mut self) -> Result<(), CompileFailure> {
        for pending in std::mem::take(&mut self.pending) {
            let product = pending.product.settle()?;
            self.record_timings(&product);
            if let Some((package, row)) = pending.supplier
                && let Some(row) = self
                    .by_package
                    .get_mut(&package)
                    .and_then(|rows| rows.get_mut(row))
            {
                row.supplier = Some(product);
            }
        }
        Ok(())
    }

    fn record_timings(&mut self, product: &project_build::PlannedProduct) {
        if product.timings().is_enabled() {
            self.timings
                .get_or_insert_with(artifacts::compile_timings::CompileTimings::enabled)
                .extend(product.timings());
        }
    }

    pub(super) fn take_timings(&mut self) -> artifacts::compile_timings::CompileTimings {
        self.timings.take().unwrap_or_default()
    }

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

    /// Open the Product of `settled` after forming the handoffs of its dependencies.
    /// Each dependency's Build is checked again on its Product's sources while
    /// the traversal continues and this Product opens.
    ///
    /// With `alongside`, this Product's own Build recheck and those of
    /// dependencies that supply no component may still be running when this
    /// returns: the caller checks the Product while they finish and settles
    /// them before reporting it. Otherwise, and whenever this Product fails to
    /// open, every check finishes first. Either way a dependency's refusal is
    /// reported before this Product's own.
    pub(super) fn dependency_product(
        &self,
        settled: &project_build::SettledDependency,
        purpose: DependencyPurpose,
        target: target::TargetProfile,
        products: &mut DependencyProducts,
        scope: project_build::DependencyCheck,
        alongside: bool,
    ) -> Result<(project_build::PlannedProduct, DependencyRechecks), CompileFailure> {
        let alongside = alongside && project_build::rechecks_outlive_caller();
        project_build::with_build_rechecks(|| {
            let opened = self
                .dependency_inputs(settled, purpose, target, products)
                .and_then(|inputs| settled.open_pending(inputs, scope));
            if alongside
                && opened.is_ok()
                && products
                    .pending
                    .iter()
                    .all(|pending| pending.supplier.is_none())
            {
                // Rechecks settle in opening order: the dependencies', then
                // this Product's own.
                let mut pending = Vec::with_capacity(products.pending.len() + 1);
                for dependency in std::mem::take(&mut products.pending) {
                    let (product, recheck) = dependency.product.release_for_check();
                    products.record_timings(&product);
                    pending.extend(recheck);
                }
                let (product, own) = opened?.release_for_check();
                pending.extend(own);
                return Ok((product, DependencyRechecks { pending }));
            }
            products.settle()?;
            Ok((opened?.settle()?, DependencyRechecks::default()))
        })
    }

    /// Open the Product of `settled` and form its Build handoff for `target`.
    /// The handoff is formed while the dependency's Build is checked again on
    /// the Product's sources, because neither reads what the other produces.
    pub(super) fn dependency_handoff(
        &self,
        settled: &project_build::SettledDependency,
        purpose: DependencyPurpose,
        target: target::TargetProfile,
        products: &mut DependencyProducts,
    ) -> Result<(project_build::PlannedProduct, PackageBuildHandoff), CompileFailure> {
        project_build::with_build_rechecks(|| {
            let opened = self
                .dependency_inputs(settled, purpose, target, products)
                .and_then(|inputs| {
                    let pending = settled
                        .open_pending(inputs, project_build::DependencyCheck::DemandedClosure)?;
                    let handoff = pending.build_handoff(target);
                    Ok((pending, handoff))
                });
            products.settle()?;
            let (pending, handoff) = opened?;
            // A refused recheck is reported before a refused handoff.
            let product = pending.settle()?;
            Ok((product, handoff?))
        })
    }

    /// Recursively form each package/purpose/target handoff once from that dependency's
    /// original Build return and the handoffs of its own dependencies.
    fn dependency_inputs(
        &self,
        settled: &project_build::SettledDependency,
        purpose: DependencyPurpose,
        target: target::TargetProfile,
        products: &mut DependencyProducts,
    ) -> Result<PackageCompilationInputs, CompileFailure> {
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
            let child_inputs =
                self.dependency_inputs(&child.build, purpose, deployment, products)?;
            let product = child.build.open_pending(
                child_inputs,
                project_build::DependencyCheck::DemandedClosure,
            )?;
            let bundle = product.build_handoff(deployment);
            // Keep the Product before its handoff can fail, so its recheck is
            // settled, and reported first, either way.
            let index = products.pending.len();
            products.pending.push(PendingDependency {
                product,
                supplier: None,
            });
            let bundle = bundle?;
            let rows = products.by_package.entry(package).or_default();
            if purpose == DependencyPurpose::Product
                && products.independent_target == Some(deployment)
            {
                products.pending[index].supplier = Some((package, rows.len()));
            }
            rows.push(CompletedDependency {
                handoff: bundle.clone(),
                supplier: None,
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
        Ok(inputs)
    }
}
