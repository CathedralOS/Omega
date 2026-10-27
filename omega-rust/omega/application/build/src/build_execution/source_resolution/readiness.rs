//! Execute each immutable Build once, preparing dependencies at readiness pauses.
//!
//! Acquisitions own transport and graph validity. This scheduler owns only live
//! continuations and returned/settled compiler work. A helper's completed graph
//! can settle early; the enclosing graph must later agree with that same source
//! custody before any Product is published.

use super::{
    CatalogMemberBuild, ProjectSourceResolutionError, ResolvedBuild, ResolvedPackageBuild,
    normal_return,
};
use crate::BuildSession;
use crate::build_execution::ReturnedPackageBuild;
use build_declarations::{BuildRequest, DependencyPurpose};
use compiler::{CheckedCompileRequest, PackageBuild};
use package_manager::resolution::source::{
    AcquiredBuildSource, AcquiredSourceSelection, BuildSourceAcquisition, BuildSourceAcquisitions,
    DeclaredBuildSource, ExecutedSourceResolution, ResolvedBuildSources,
};
use std::path::Path;
use std::sync::Arc;
use target::TargetProfile;

use ProjectSourceResolutionError::{Compilation, Resources, Source};

struct SourceBuild {
    acquired: AcquiredBuildSource,
    declared: DeclaredBuildSource,
    // Taken only during settlement or final transfer; a failed transition
    // aborts this invocation instead of replaying the original Build.
    build: Option<ResolvedBuild>,
}

struct ExecutedSourceBuild {
    build: ReturnedPackageBuild,
    dependencies: Vec<Arc<ResolvedBuildSources>>,
}

struct QueryDependencies {
    inputs: Vec<package_compilation::source_imports::ProductQueryDependency>,
    graphs: Vec<Arc<ResolvedBuildSources>>,
    selections: Vec<build_evaluation::BuildDependencySelection>,
}

/// Invocation scratch for one Product waiting on its generated-source inputs.
/// A dependency's completed bundle returns to this frame, not a recursive Rust
/// call. The same source may still have distinct Build and Product occurrences.
struct PendingProduct {
    source: usize,
    purpose: DependencyPurpose,
    inputs: package_compilation::PackageCompilationInputs,
    dependencies: std::collections::btree_set::IntoIter<(
        semantic_vocabulary::PackageKeyIdentity,
        DependencyPurpose,
    )>,
    generated: Vec<package_compilation::PackageGeneratedSourceBundle>,
}

pub(super) struct ReadinessScheduler<'session, 'operation> {
    session: &'session mut BuildSession,
    acquisitions: &'session mut BuildSourceAcquisitions<'operation>,
    profile: TargetProfile,
    timings: bool,
    active: Vec<AcquiredBuildSource>,
    sources: Vec<SourceBuild>,
    products: Vec<(usize, DependencyPurpose, PackageBuild)>,
    catalog_builds: Vec<CatalogMemberBuild>,
}

impl<'session, 'operation> ReadinessScheduler<'session, 'operation> {
    pub(super) fn new(
        session: &'session mut BuildSession,
        acquisitions: &'session mut BuildSourceAcquisitions<'operation>,
        profile: TargetProfile,
        timings: bool,
    ) -> Self {
        Self {
            session,
            acquisitions,
            profile,
            timings,
            active: Vec::new(),
            sources: Vec::new(),
            products: Vec::new(),
            catalog_builds: Vec::new(),
        }
    }

    pub(super) fn join_sources(
        self,
        resolved: &ResolvedBuildSources,
    ) -> Result<Vec<ResolvedPackageBuild>, ProjectSourceResolutionError> {
        let mut builds = Vec::with_capacity(self.sources.len());
        for mut source in self.sources {
            let identity = source.declared.identity();
            let mut build = source
                .build
                .take()
                .ok_or_else(|| Source("Build work was already consumed".into()))?;
            if !resolved.contains(identity) {
                // Unselected members are declarations, not Product roots. Only
                // their actually demanded dependency closures can account for a
                // source absent from the selected Product graph.
                let mut observed = false;
                for catalog in &self.catalog_builds {
                    for graph in catalog.observation.dependency_graphs() {
                        if graph.contains(identity) {
                            join_source(graph, identity, &mut build)?;
                            observed = true;
                        }
                    }
                }
                if !observed {
                    return Err(Source(
                        "demanded dependency is absent from the final source graph".into(),
                    ));
                }
                continue;
            }
            let inputs = join_source(resolved, identity, &mut build)?;
            builds.push(ResolvedPackageBuild { build, inputs });
        }
        Ok(builds)
    }

    pub(super) fn execute_source(
        &mut self,
        acquired: AcquiredBuildSource,
        entry: &Path,
        execution: package_compilation::BuildExecutionInputs,
    ) -> Result<usize, ProjectSourceResolutionError> {
        if let Some(position) = self
            .sources
            .iter()
            .position(|work| work.acquired.same_source(&acquired))
        {
            if self.sources[position].acquired.source_capture() != acquired.source_capture() {
                return Err(Source(
                    "reused Build source changed its immutable capture".into(),
                ));
            }
            return Ok(position);
        }
        self.acquisitions
            .admit_build_descent(&self.active, &acquired)
            .map_err(Source)?;
        self.active.push(acquired.clone());
        let result = (|| {
            let build = if let Some(position) = self
                .catalog_builds
                .iter()
                .position(|work| &work.capture == acquired.source_capture())
            {
                self.catalog_builds.swap_remove(position).build
            } else {
                let mut request = self
                    .session
                    .packages
                    .source_build_request(acquired.source_capture(), entry, self.profile, execution)
                    .map_err(Resources)?;
                request.collect_timings = self.timings;
                self.execute_build(&acquired, request)?.build
            };
            let declared = acquired
                .declare_build(normal_return(&build)?)
                .map_err(Source)?;
            let position = self.sources.len();
            self.sources.push(SourceBuild {
                acquired,
                declared,
                build: Some(ResolvedBuild::Returned(build)),
            });
            Ok(position)
        })();
        self.active.pop();
        result
    }

    fn execute_build(
        &mut self,
        acquired: &AcquiredBuildSource,
        request: CheckedCompileRequest,
    ) -> Result<ExecutedSourceBuild, ProjectSourceResolutionError> {
        let mut dependency_graphs = Vec::new();
        let mut invocation = self
            .session
            .compiler
            .begin_package_build(request)
            .map_err(Compilation)?;
        let mut queries = invocation.product_queries();
        let returned = if let Some(execution) = invocation.execution() {
            let mut host = execution
                .host()
                .map_err(|diagnostics| Compilation(diagnostics.into()))?;
            let mut activation = host.start().map_err(Source)?;
            loop {
                let step = activation.advance(1024).map_err(Source)?;
                if let Some(query) = step.awaiting_query {
                    let dependencies =
                        self.prepare_query_dependencies(acquired, activation.requests())?;
                    let description = queries
                        .describe(&query, dependencies.inputs)
                        .map_err(|diagnostics| Compilation(diagnostics.into()))?;
                    activation
                        .answer_query(&query, description, &dependencies.selections)
                        .map_err(Source)?;
                    dependency_graphs.extend(dependencies.graphs);
                    continue;
                }
                if step.returned {
                    break;
                }
                if let Some(request) = step.awaiting_provider {
                    let (alias, declaration) =
                        request.backing.path.split_once("::").ok_or_else(|| {
                            Source(
                                "Build backing must name its dependency and implementation".into(),
                            )
                        })?;
                    let (ordinal, dependency) =
                        self.prepare_dependency(acquired, activation.requests(), alias)?;
                    let resolved = Arc::new(self.resolve_graph(dependency)?);
                    self.settle_graph(&resolved)?;
                    let build = self.prepare_product(dependency, DependencyPurpose::Build)?;
                    let provider = self
                        .session
                        .compiler
                        .produce_package_build_provider(build, declaration)
                        .map_err(Compilation)?;
                    activation
                        .install_provider(
                            build_evaluation::BuildDependencySelection {
                                purpose: DependencyPurpose::Build,
                                request_ordinal: ordinal,
                                package: self.sources[dependency].declared.identity(),
                            },
                            &provider,
                        )
                        .map_err(Source)?;
                    dependency_graphs.push(resolved);
                }
            }
            Some(activation.complete().map_err(Source)?)
        } else {
            None
        };
        Ok(ExecutedSourceBuild {
            build: ReturnedPackageBuild {
                invocation,
                returned,
            },
            dependencies: dependency_graphs,
        })
    }

    fn prepare_dependency(
        &mut self,
        requester: &AcquiredBuildSource,
        requests: &[build_evaluation::terminal_requests::IssuedBuildRequest],
        alias: &str,
    ) -> Result<(usize, usize), ProjectSourceResolutionError> {
        let mut selected = None;
        let dependencies = requests.iter().filter_map(|issued| match issued.request() {
            BuildRequest::Dependency {
                purpose: DependencyPurpose::Build,
                alias,
                ..
            } => Some(alias),
            _ => None,
        });
        for (ordinal, explicit_alias) in dependencies.enumerate() {
            if explicit_alias
                .as_ref()
                .is_some_and(|name| name.as_str() != alias)
            {
                continue;
            }
            let selection = self
                .acquisitions
                .acquire_issued(requester, requests, DependencyPurpose::Build, ordinal)
                .map_err(Source)?;
            let acquired = self.finish_acquisition(selection)?;
            let entry = acquired.entry_path();
            let dependency = self.execute_source(acquired, &entry, Default::default())?;
            let declared = &self.sources[dependency].declared;
            declared.require_dependency().map_err(Source)?;
            let name = explicit_alias
                .as_ref()
                .map_or(declared.package_name(), |name| name.as_str());
            if name == alias && selected.replace((ordinal, dependency)).is_some() {
                return Err(Source(format!(
                    "Build dependency qualifier `{alias}` is ambiguous"
                )));
            }
        }
        selected.ok_or_else(|| {
            Source(format!(
                "Build provider qualifier `{alias}` has no issued Build dependency"
            ))
        })
    }

    fn prepare_query_dependencies(
        &mut self,
        requester: &AcquiredBuildSource,
        requests: &[build_evaluation::terminal_requests::IssuedBuildRequest],
    ) -> Result<QueryDependencies, ProjectSourceResolutionError> {
        let mut dependencies = Vec::new();
        let mut graphs = Vec::new();
        let mut selections = Vec::new();
        let issued = requests.iter().filter_map(|issued| match issued.request() {
            BuildRequest::Dependency {
                purpose: DependencyPurpose::Product,
                alias,
                ..
            } => Some(alias),
            _ => None,
        });
        for (ordinal, alias) in issued.enumerate() {
            let selection = self
                .acquisitions
                .acquire_issued(requester, requests, DependencyPurpose::Product, ordinal)
                .map_err(Source)?;
            let acquired = self.finish_acquisition(selection)?;
            let entry = acquired.entry_path();
            let dependency = self.execute_source(acquired, &entry, Default::default())?;
            self.sources[dependency]
                .declared
                .require_dependency()
                .map_err(Source)?;
            let resolved = Arc::new(self.resolve_graph(dependency)?);
            self.settle_graph(&resolved)?;
            let declared = &self.sources[dependency].declared;
            dependencies.push(
                package_compilation::source_imports::ProductQueryDependency {
                    alias: alias.as_ref().map_or_else(
                        || declared.package_name().to_owned(),
                        |alias| alias.as_str().to_owned(),
                    ),
                    inputs: resolved
                        .compilation_inputs(declared.identity())
                        .map_err(Source)?,
                },
            );
            graphs.push(resolved);
            selections.push(build_evaluation::BuildDependencySelection {
                purpose: DependencyPurpose::Product,
                request_ordinal: ordinal,
                package: declared.identity(),
            });
        }
        Ok(QueryDependencies {
            inputs: dependencies,
            graphs,
            selections,
        })
    }

    fn finish_acquisition(
        &mut self,
        selection: AcquiredSourceSelection,
    ) -> Result<AcquiredBuildSource, ProjectSourceResolutionError> {
        match selection {
            AcquiredSourceSelection::Source(source) => Ok(source),
            AcquiredSourceSelection::Workspace(workspace) => {
                self.execute_workspace_catalog(workspace)
            }
        }
    }

    fn execute_workspace_catalog(
        &mut self,
        workspace: package_manager::resolution::source::AcquiredWorkspaceBuild,
    ) -> Result<AcquiredBuildSource, ProjectSourceResolutionError> {
        let acquired = workspace.acquired_source().clone();
        self.acquisitions
            .admit_build_descent(&self.active, &acquired)
            .map_err(Source)?;
        self.active.push(acquired.clone());
        let result = (|| {
            let mut request = self
                .session
                .packages
                .source_build_request(
                    acquired.source_capture(),
                    &workspace.entry_path(),
                    self.profile,
                    Default::default(),
                )
                .map_err(Resources)?;
            request.collect_timings = self.timings;
            let build = self.execute_build(&acquired, request)?.build;
            let returned = build
                .returned
                .ok_or_else(|| Source("workspace catalog has no executed declaration".into()))?;
            let root = self
                .session
                .compiler
                .settle_source_catalog(build.invocation, returned)
                .map_err(Compilation)?;
            let members = self
                .acquisitions
                .workspace_members(workspace, root)
                .map_err(Source)?;
            let mut declarations = Vec::new();
            for acquired in members.sources() {
                let mut request = self
                    .session
                    .packages
                    .source_build_request(
                        acquired.source_capture(),
                        &acquired.entry_path(),
                        self.profile,
                        Default::default(),
                    )
                    .map_err(Resources)?;
                request.collect_timings = self.timings;
                self.acquisitions
                    .admit_build_descent(&self.active, acquired)
                    .map_err(Source)?;
                self.active.push(acquired.clone());
                let executed = self.execute_build(acquired, request);
                self.active.pop();
                let executed = executed?;
                let build = executed.build;
                let declaration = build
                    .invocation
                    .observe_declaration(normal_return(&build)?)
                    .map_err(Compilation)?;
                let observation =
                    package_manager::resolution::source::WorkspaceMemberObservation::new(
                        acquired,
                        declaration,
                        normal_return(&build)?,
                        executed.dependencies,
                    )
                    .map_err(Source)?;
                declarations.push(observation.clone());
                self.catalog_builds.push(CatalogMemberBuild {
                    capture: acquired.source_capture().clone(),
                    build,
                    observation,
                });
            }
            self.acquisitions
                .complete_workspace(members, declarations)
                .map_err(Source)
        })();
        self.active.pop();
        result
    }

    pub(super) fn resolve_graph(
        &mut self,
        root: usize,
    ) -> Result<ResolvedBuildSources, ProjectSourceResolutionError> {
        let mut resolution = self.sources[root]
            .declared
            .clone()
            .resolve()
            .map_err(Source)?;
        loop {
            let request = match resolution {
                ExecutedSourceResolution::Complete(resolved) => return Ok(resolved),
                ExecutedSourceResolution::Request(request) => request,
            };
            let mut acquisition = self.acquisitions.acquire(request).map_err(Source)?;
            loop {
                match acquisition {
                    BuildSourceAcquisition::Execute(dependency) => {
                        let acquired = dependency.acquired_source().clone();
                        let entry = acquired.entry_path();
                        let position = self.execute_source(acquired, &entry, Default::default())?;
                        let (_, next) = dependency
                            .complete_declared(&self.sources[position].declared)
                            .map_err(Source)?;
                        resolution = next;
                        break;
                    }
                    BuildSourceAcquisition::Reused(next) => {
                        resolution = next;
                        break;
                    }
                    BuildSourceAcquisition::Workspace { pending, workspace } => {
                        let acquired =
                            self.finish_acquisition(AcquiredSourceSelection::Workspace(workspace))?;
                        acquisition = pending.select(acquired).map_err(Source)?;
                    }
                }
            }
        }
    }

    fn settle_graph(
        &mut self,
        resolved: &ResolvedBuildSources,
    ) -> Result<(), ProjectSourceResolutionError> {
        for source in &mut self.sources {
            let identity = source.declared.identity();
            if !resolved.contains(identity) {
                continue;
            }
            let mut build = source
                .build
                .take()
                .ok_or_else(|| Source("Build work was already consumed".into()))?;
            let inputs = join_source(resolved, identity, &mut build)?;
            if let ResolvedBuild::Returned(mut returned) = build {
                returned
                    .invocation
                    .join_product_context(self.profile, inputs)
                    .map_err(Compilation)?;
                build = ResolvedBuild::Settled(
                    self.session
                        .compiler
                        .settle_package_build(returned.invocation, returned.returned)
                        .map_err(Compilation)?,
                );
            }
            source.build = Some(build);
        }
        Ok(())
    }

    fn prepare_product(
        &mut self,
        position: usize,
        purpose: DependencyPurpose,
    ) -> Result<PackageBuild, ProjectSourceResolutionError> {
        if let Some(prepared) = self.prepared_product(position, purpose)? {
            return Ok(prepared);
        }
        let mut pending = vec![self.pending_product(position, purpose)?];
        while let Some(current) = pending.last_mut() {
            if let Some((package, purpose)) = current.dependencies.next() {
                let dependency = self
                    .sources
                    .iter()
                    .position(|source| source.declared.identity() == package)
                    .ok_or_else(|| Source("helper dependency has no retained Build".into()))?;
                if let Some(prepared) = self.prepared_product(dependency, purpose)? {
                    current
                        .generated
                        .push(prepared.generated_source_bundle().clone());
                } else {
                    pending.push(self.pending_product(dependency, purpose)?);
                }
                continue;
            }

            let current = pending
                .pop()
                .ok_or_else(|| Source("helper preparation lost its pending Product".into()))?;
            let prepared = self.complete_product(current)?;
            match pending.last_mut() {
                Some(parent) => parent
                    .generated
                    .push(prepared.generated_source_bundle().clone()),
                None => return Ok(prepared),
            }
        }
        Err(Source("helper preparation has no Product root".into()))
    }

    fn prepared_product(
        &self,
        position: usize,
        purpose: DependencyPurpose,
    ) -> Result<Option<PackageBuild>, ProjectSourceResolutionError> {
        self.products
            .iter()
            .find(|(source, scope, _)| *source == position && *scope == purpose)
            .map(|(_, _, prepared)| {
                prepared.select_target(self.profile, prepared.package_inputs().clone())
            })
            .transpose()
            .map_err(Compilation)
    }

    fn pending_product(
        &self,
        position: usize,
        purpose: DependencyPurpose,
    ) -> Result<PendingProduct, ProjectSourceResolutionError> {
        let Some(ResolvedBuild::Settled(completed)) = &self.sources[position].build else {
            return Err(Source(
                "helper preparation requires its settled dependency graph".into(),
            ));
        };
        let inputs = completed
            .package_inputs()
            .clone()
            .with_compilation_purpose(purpose)
            .map_err(|errors| Source(format!("helper purpose admission: {errors:?}")))?;
        let dependencies = inputs.required_dependency_source_instances().into_iter();
        Ok(PendingProduct {
            source: position,
            purpose,
            inputs,
            generated: Vec::with_capacity(dependencies.len()),
            dependencies,
        })
    }

    fn complete_product(
        &mut self,
        pending: PendingProduct,
    ) -> Result<PackageBuild, ProjectSourceResolutionError> {
        let PendingProduct {
            source,
            purpose,
            inputs,
            generated,
            ..
        } = pending;
        let inputs = inputs
            .with_complete_dependency_generated_sources(generated)
            .map_err(|errors| Source(format!("helper generated source join: {errors:?}")))?;
        let Some(ResolvedBuild::Settled(completed)) = &mut self.sources[source].build else {
            return Err(Source(
                "helper preparation consumed its settled Build".into(),
            ));
        };
        let prepared = self
            .session
            .compiler
            .prepare_package_product(completed, self.profile, inputs, Default::default())
            .map_err(Compilation)?;
        let retained = prepared
            .select_target(self.profile, prepared.package_inputs().clone())
            .map_err(Compilation)?;
        self.products.push((source, purpose, retained));
        Ok(prepared)
    }
}

fn join_source(
    resolved: &ResolvedBuildSources,
    identity: semantic_vocabulary::PackageKeyIdentity,
    build: &mut ResolvedBuild,
) -> Result<package_compilation::PackageCompilationInputs, ProjectSourceResolutionError> {
    match build {
        ResolvedBuild::Returned(build) => {
            let returned = build.returned.as_mut().ok_or_else(|| {
                Source("acquired source has no executed Build declaration".into())
            })?;
            resolved.join_build(identity, returned).map_err(Source)
        }
        ResolvedBuild::Settled(build) => {
            let inputs = resolved.compilation_inputs(identity).map_err(Source)?;
            if inputs.source_inputs() != build.package_inputs().source_inputs() {
                return Err(Source(
                    "settled helper Build differs from the final source graph".into(),
                ));
            }
            Ok(inputs)
        }
    }
}
