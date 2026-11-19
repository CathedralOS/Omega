//! Acquire the exact dependency occurrences requested by a suspended Build.
use super::SourceHost;
use build_declarations::{BuildRequest, DependencyPurpose};
use build_execution::host::bindings::{BuildProductQuery, BuildProviderRequest};
use build_execution::host::requests::BuildDependencyPreparation;
use build_execution::{
    BuildDependencySelection, CompileFailure, ProducedBuildProvider, TerminalBuildActivation,
};
use diagnostics::Diagnostic;
use package_compilation::source_imports::ProductQueryDependency;
use package_compilation::{PackageBuildHandoff, PackageCompilationInputs};
use package_manager::PackageCommandError;
use package_manager::resolution::source::{AcquiredBuildSource, AcquiredSourceSelection};
use project_build::planning::PlanningServices;

pub(super) struct SourceServices<'a, 's, 'p> {
    pub host: &'a mut SourceHost<'s, 'p>,
    pub acquired: &'a AcquiredBuildSource,
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
        self.host
            .acquisitions
            .admit_build_descent(&self.host.active, &child)
            .map_err(refused)?;
        if let Some(i) = self
            .host
            .settled
            .iter()
            .position(|(source, _, _)| source.same_source(&child))
        {
            child
                .verify_reused_capture(&self.host.settled[i].0)
                .map_err(refused)?;
            return Ok(i);
        }
        let stage = self.host.member_or_execute(&child).map_err(failure)?;
        let declared = child.declare_build(stage.returned()).map_err(refused)?;
        declared.require_dependency().map_err(refused)?;
        let resolved = self.host.join(child.clone(), stage).map_err(failure)?;
        let settled = resolved.root.settle_dependency(resolved.inputs)?;
        self.host
            .session
            .record_source_completion(settled.evaluated().source_completion_accounting.clone());
        let i = self.host.settled.len();
        self.host.settled.push((child, declared, settled));
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
        let (_, declaration, settled) = &self.host.settled[i];
        let alias = aliases(activation, request.purpose)
            .nth(request.request_ordinal)
            .ok_or_else(|| refused("Build preparation names no issued dependency"))?
            .unwrap_or_else(|| declaration.package_name())
            .to_owned();
        let product = self.host.handoffs().dependency_product(
            settled,
            request.purpose,
            self.host.profile,
            &mut Vec::new(),
        )?;
        Ok(ProductQueryDependency {
            alias,
            inputs: product
                .package_inputs()
                .ok_or_else(|| refused("prepared dependency lost its package source"))?
                .clone(),
            generated: Some(product.build_handoff(self.host.profile)?),
        })
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
                let (_, declaration, settled) = &self.host.settled[i];
                Ok(ProductQueryDependency {
                    alias: alias
                        .unwrap_or_else(|| declaration.package_name())
                        .to_owned(),
                    inputs: settled.inputs().clone(),
                    generated: None,
                })
            })
            .collect()
    }

    fn provide(
        &mut self,
        activation: &TerminalBuildActivation,
        request: &BuildProviderRequest,
    ) -> Result<(BuildDependencySelection, ProducedBuildProvider), CompileFailure> {
        let (qualifier, path) =
            request.backing.path.split_once("::").ok_or_else(|| {
                refused("Build backing must name its dependency and implementation")
            })?;
        let purpose = DependencyPurpose::Build;
        let mut selected = None;
        for (ordinal, alias) in aliases(activation, purpose).enumerate() {
            if alias.is_some_and(|alias| alias != qualifier) {
                continue;
            }
            let i = self.dependency(activation, purpose, ordinal)?;
            let name = self.host.settled[i].1.package_name();
            if alias.unwrap_or(name) == qualifier && selected.replace((ordinal, i)).is_some() {
                return Err(refused(format!(
                    "Build dependency qualifier `{qualifier}` is ambiguous"
                )));
            }
        }
        let (request_ordinal, i) = selected.ok_or_else(|| {
            refused(format!(
                "Build provider qualifier `{qualifier}` has no issued Build dependency"
            ))
        })?;
        let (_, declared, settled) = &self.host.settled[i];
        let profile = self.host.profile;
        let product =
            self.host
                .handoffs()
                .dependency_product(settled, purpose, profile, &mut Vec::new())?;
        let provider = product
            .product_stage(&[profile])?
            .produce_build_provider(profile, path)?;
        Ok((
            BuildDependencySelection {
                purpose,
                request_ordinal,
                package: declared.identity(),
            },
            provider,
        ))
    }
}

pub(super) struct HandoffSources<'a> {
    pub profile: target::TargetProfile,
    pub settled: &'a [super::SettledSource],
}

impl SourceHost<'_, '_> {
    fn handoffs(&self) -> HandoffSources<'_> {
        HandoffSources {
            profile: self.profile,
            settled: &self.settled,
        }
    }
}

impl HandoffSources<'_> {
    /// Recursively form each package/purpose/target handoff once from that dependency's
    /// original Build return and the handoffs of its own dependencies.
    pub(super) fn dependency_product(
        &self,
        settled: &project_build::planning::SettledDependency,
        purpose: DependencyPurpose,
        target: target::TargetProfile,
        cache: &mut Vec<PackageBuildHandoff>,
    ) -> Result<project_build::planning::PlannedProduct, CompileFailure> {
        let inputs = settled
            .inputs()
            .clone()
            .with_compilation_purpose(purpose)
            .map_err(|errors| {
                refused(format!(
                    "cannot select dependency compilation purpose: {errors:?}"
                ))
            })?;
        let mut bundles = Vec::new();
        for (package, purpose) in inputs.required_dependency_source_instances() {
            let deployment = match purpose {
                DependencyPurpose::Product => target,
                DependencyPurpose::Build => self.profile,
            };
            if let Some(handoff) = cache.iter().find(|bundle| {
                bundle.package() == package
                    && bundle.purpose() == purpose
                    && bundle.target() == deployment
            }) {
                bundles.push(handoff.clone());
                continue;
            }
            let (_, _, child) = self
                .settled
                .iter()
                .find(|(_, declared, _)| declared.identity() == package)
                .ok_or_else(|| refused("dependency graph has no corresponding settled Build"))?;
            let product = self.dependency_product(child, purpose, deployment, cache)?;
            let bundle = product.build_handoff(deployment)?;
            cache.push(bundle.clone());
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
