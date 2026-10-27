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
        self.run_work(Work::Source {
            acquired,
            entry: entry.to_path_buf(),
            execution,
        })?
        .source()
    }

    pub(super) fn resolve_graph(
        &mut self,
        root: usize,
    ) -> Result<ResolvedBuildSources, ProjectSourceResolutionError> {
        let resolution = self.sources[root]
            .declared
            .clone()
            .resolve()
            .map_err(Source)?;
        self.run_work(Work::Graph(resolution))?.graph()
    }

    /// Dependency pauses keep their activation in the pending work, not on the
    /// Rust call stack. Every child completes before its parent resumes; a failure
    /// drops this invocation's pending jobs without replaying any Build.
    fn run_work(&mut self, root: Work) -> Result<CompletedWork, ProjectSourceResolutionError> {
        let active_depth = self.active.len();
        let result = self.drive_work(root);
        self.active.truncate(active_depth);
        result
    }

    fn drive_work(&mut self, root: Work) -> Result<CompletedWork, ProjectSourceResolutionError> {
        let mut pending = vec![root];
        let mut completed = None;
        while let Some(work) = pending.pop() {
            match work {
                Work::Source {
                    acquired,
                    entry,
                    execution,
                } => {
                    if let Some(position) = self
                        .sources
                        .iter()
                        .position(|work| work.acquired.same_source(&acquired))
                    {
                        if self.sources[position].acquired.source_capture()
                            != acquired.source_capture()
                        {
                            return Err(Source(
                                "reused Build source changed its immutable capture".into(),
                            ));
                        }
                        completed = Some(CompletedWork::Source(position));
                        continue;
                    }
                    self.acquisitions
                        .admit_build_descent(&self.active, &acquired)
                        .map_err(Source)?;
                    self.active.push(acquired.clone());
                    if let Some(position) = self
                        .catalog_builds
                        .iter()
                        .position(|work| &work.capture == acquired.source_capture())
                    {
                        completed = Some(CompletedWork::Build(ExecutedSourceBuild {
                            build: self.catalog_builds.swap_remove(position).build,
                            dependencies: Vec::new(),
                        }));
                        pending.push(Work::Declare(acquired));
                    } else {
                        let request = self.build_request(&acquired, &entry, execution)?;
                        pending.push(Work::Declare(acquired.clone()));
                        pending.push(Work::Build { acquired, request });
                    }
                }
                Work::Declare(acquired) => {
                    let build = take_completed(&mut completed)?.build()?.build;
                    let declared = acquired
                        .declare_build(normal_return(&build)?)
                        .map_err(Source)?;
                    let position = self.sources.len();
                    self.sources.push(SourceBuild {
                        acquired,
                        declared,
                        build: Some(ResolvedBuild::Returned(build)),
                    });
                    self.active.pop();
                    completed = Some(CompletedWork::Source(position));
                }
                Work::Build { acquired, request } => {
                    let mut invocation = self
                        .session
                        .compiler
                        .begin_package_build(request)
                        .map_err(Compilation)?;
                    let queries = invocation.product_queries();
                    let activation = match invocation.execution() {
                        Some(execution) => Some(
                            execution
                                .host()
                                .map_err(|diagnostics| Compilation(diagnostics.into()))?
                                .start()
                                .map_err(Source)?,
                        ),
                        None => None,
                    };
                    if let Some(activation) = activation {
                        pending.push(Work::Run(Box::new(LiveBuild {
                            acquired,
                            invocation,
                            queries,
                            activation,
                            dependencies: Vec::new(),
                        })));
                    } else {
                        completed = Some(CompletedWork::Build(ExecutedSourceBuild {
                            build: ReturnedPackageBuild {
                                invocation,
                                returned: None,
                            },
                            dependencies: Vec::new(),
                        }));
                    }
                }
                Work::Run(mut build) => {
                    let step = build.activation.advance(1024).map_err(Source)?;
                    if let Some(query) = step.awaiting_query {
                        pending.push(Work::Dependencies(PendingDependencies::new(
                            build,
                            Demand::Query {
                                query,
                                dependencies: QueryDependencies {
                                    inputs: Vec::new(),
                                    graphs: Vec::new(),
                                    selections: Vec::new(),
                                },
                            },
                        )));
                    } else if step.returned {
                        let LiveBuild {
                            invocation,
                            activation,
                            dependencies,
                            ..
                        } = *build;
                        completed = Some(CompletedWork::Build(ExecutedSourceBuild {
                            build: ReturnedPackageBuild {
                                invocation,
                                returned: Some(activation.complete().map_err(Source)?),
                            },
                            dependencies,
                        }));
                    } else if let Some(request) = step.awaiting_provider {
                        let (alias, declaration) =
                            request.backing.path.split_once("::").ok_or_else(|| {
                                Source(
                                    "Build backing must name its dependency and implementation"
                                        .into(),
                                )
                            })?;
                        pending.push(Work::Dependencies(PendingDependencies::new(
                            build,
                            Demand::Provider {
                                alias: alias.to_owned(),
                                declaration: declaration.to_owned(),
                                selected: None,
                            },
                        )));
                    } else {
                        pending.push(Work::Run(build));
                    }
                }
                Work::Dependencies(mut dependencies) => {
                    if let Some((ordinal, alias)) = dependencies.requests.next() {
                        let selection = self
                            .acquisitions
                            .acquire_issued(
                                &dependencies.build.acquired,
                                dependencies.build.activation.requests(),
                                dependencies.demand.purpose(),
                                ordinal,
                            )
                            .map_err(Source)?;
                        pending.push(Work::DependencySource {
                            dependencies,
                            ordinal,
                            alias,
                        });
                        pending.push(Work::ExecuteAcquired);
                        pending.push(Work::Acquire(selection));
                    } else {
                        match dependencies.demand {
                            Demand::Provider {
                                alias,
                                declaration,
                                selected,
                            } => {
                                let (ordinal, source) = selected.ok_or_else(|| Source(format!(
                                    "Build provider qualifier `{alias}` has no issued Build dependency"
                                )))?;
                                let resolution = self.sources[source]
                                    .declared
                                    .clone()
                                    .resolve()
                                    .map_err(Source)?;
                                pending.push(Work::InstallProvider {
                                    build: dependencies.build,
                                    declaration,
                                    ordinal,
                                    source,
                                });
                                pending.push(Work::Graph(resolution));
                            }
                            Demand::Query {
                                query,
                                dependencies: ready,
                            } => {
                                let mut build = dependencies.build;
                                let description = build
                                    .queries
                                    .describe(&query, ready.inputs)
                                    .map_err(|diagnostics| Compilation(diagnostics.into()))?;
                                build
                                    .activation
                                    .answer_query(&query, description, &ready.selections)
                                    .map_err(Source)?;
                                build.dependencies.extend(ready.graphs);
                                pending.push(Work::Run(build));
                            }
                        }
                    }
                }
                Work::DependencySource {
                    mut dependencies,
                    ordinal,
                    alias,
                } => {
                    let source = take_completed(&mut completed)?.source()?;
                    let declared = &self.sources[source].declared;
                    declared.require_dependency().map_err(Source)?;
                    let name = alias.unwrap_or_else(|| declared.package_name().to_owned());
                    match &mut dependencies.demand {
                        Demand::Provider {
                            alias, selected, ..
                        } => {
                            if name == *alias && selected.replace((ordinal, source)).is_some() {
                                return Err(Source(format!(
                                    "Build dependency qualifier `{alias}` is ambiguous"
                                )));
                            }
                            pending.push(Work::Dependencies(dependencies));
                        }
                        Demand::Query { .. } => {
                            let resolution = declared.clone().resolve().map_err(Source)?;
                            pending.push(Work::QueryGraph {
                                dependencies,
                                ordinal,
                                source,
                                alias: name,
                            });
                            pending.push(Work::Graph(resolution));
                        }
                    }
                }
                Work::QueryGraph {
                    mut dependencies,
                    ordinal,
                    source,
                    alias,
                } => {
                    let graph = Arc::new(take_completed(&mut completed)?.graph()?);
                    self.settle_graph(&graph)?;
                    let declared = &self.sources[source].declared;
                    let Demand::Query {
                        dependencies: ready,
                        ..
                    } = &mut dependencies.demand
                    else {
                        return Err(Source(
                            "query dependency completed for a provider demand".into(),
                        ));
                    };
                    ready.inputs.push(
                        package_compilation::source_imports::ProductQueryDependency {
                            alias,
                            inputs: graph
                                .compilation_inputs(declared.identity())
                                .map_err(Source)?,
                        },
                    );
                    ready.graphs.push(graph);
                    ready
                        .selections
                        .push(build_evaluation::BuildDependencySelection {
                            purpose: DependencyPurpose::Product,
                            request_ordinal: ordinal,
                            package: declared.identity(),
                        });
                    pending.push(Work::Dependencies(dependencies));
                }
                Work::InstallProvider {
                    mut build,
                    declaration,
                    ordinal,
                    source,
                } => {
                    let graph = Arc::new(take_completed(&mut completed)?.graph()?);
                    self.settle_graph(&graph)?;
                    let product = self.prepare_product(source, DependencyPurpose::Build)?;
                    let provider = self
                        .session
                        .compiler
                        .produce_package_build_provider(product, &declaration)
                        .map_err(Compilation)?;
                    build
                        .activation
                        .install_provider(
                            build_evaluation::BuildDependencySelection {
                                purpose: DependencyPurpose::Build,
                                request_ordinal: ordinal,
                                package: self.sources[source].declared.identity(),
                            },
                            &provider,
                        )
                        .map_err(Source)?;
                    build.dependencies.push(graph);
                    pending.push(Work::Run(build));
                }
                Work::Acquire(selection) => match selection {
                    AcquiredSourceSelection::Source(source) => {
                        completed = Some(CompletedWork::Acquired(source))
                    }
                    AcquiredSourceSelection::Workspace(workspace) => {
                        let acquired = workspace.acquired_source().clone();
                        self.acquisitions
                            .admit_build_descent(&self.active, &acquired)
                            .map_err(Source)?;
                        self.active.push(acquired.clone());
                        let request = self.build_request(
                            &acquired,
                            &workspace.entry_path(),
                            Default::default(),
                        )?;
                        pending.push(Work::WorkspaceRoot(workspace));
                        pending.push(Work::Build { acquired, request });
                    }
                },
                Work::ExecuteAcquired => {
                    let acquired = take_completed(&mut completed)?.acquired()?;
                    let entry = acquired.entry_path();
                    pending.push(Work::Source {
                        acquired,
                        entry,
                        execution: Default::default(),
                    });
                }
                Work::WorkspaceRoot(workspace) => {
                    let build = take_completed(&mut completed)?.build()?.build;
                    let returned = build.returned.ok_or_else(|| {
                        Source("workspace catalog has no executed declaration".into())
                    })?;
                    let root = self
                        .session
                        .compiler
                        .settle_source_catalog(build.invocation, returned)
                        .map_err(Compilation)?;
                    let members = self
                        .acquisitions
                        .workspace_members(workspace, root)
                        .map_err(Source)?;
                    pending.push(Work::WorkspaceMembers(PendingWorkspace {
                        members,
                        next_member: 0,
                        declarations: Vec::new(),
                    }));
                }
                Work::WorkspaceMembers(mut workspace) => {
                    let acquired = workspace
                        .members
                        .sources()
                        .nth(workspace.next_member)
                        .cloned();
                    if let Some(acquired) = acquired {
                        workspace.next_member += 1;
                        let request = self.build_request(
                            &acquired,
                            &acquired.entry_path(),
                            Default::default(),
                        )?;
                        self.acquisitions
                            .admit_build_descent(&self.active, &acquired)
                            .map_err(Source)?;
                        self.active.push(acquired.clone());
                        pending.push(Work::WorkspaceMember {
                            workspace,
                            acquired: acquired.clone(),
                        });
                        pending.push(Work::Build { acquired, request });
                    } else {
                        let acquired = self
                            .acquisitions
                            .complete_workspace(workspace.members, workspace.declarations)
                            .map_err(Source)?;
                        self.active.pop();
                        completed = Some(CompletedWork::Acquired(acquired));
                    }
                }
                Work::WorkspaceMember {
                    mut workspace,
                    acquired,
                } => {
                    let executed = take_completed(&mut completed)?.build()?;
                    self.active.pop();
                    let build = executed.build;
                    let declaration = build
                        .invocation
                        .observe_declaration(normal_return(&build)?)
                        .map_err(Compilation)?;
                    let observation =
                        package_manager::resolution::source::WorkspaceMemberObservation::new(
                            &acquired,
                            declaration,
                            normal_return(&build)?,
                            executed.dependencies,
                        )
                        .map_err(Source)?;
                    workspace.declarations.push(observation.clone());
                    self.catalog_builds.push(CatalogMemberBuild {
                        capture: acquired.source_capture().clone(),
                        build,
                        observation,
                    });
                    pending.push(Work::WorkspaceMembers(workspace));
                }
                Work::Graph(resolution) => match resolution {
                    ExecutedSourceResolution::Complete(graph) => {
                        completed = Some(CompletedWork::Graph(graph))
                    }
                    ExecutedSourceResolution::Request(request) => {
                        let acquisition = self.acquisitions.acquire(request).map_err(Source)?;
                        pending.push(Work::GraphAcquisition(acquisition));
                    }
                },
                Work::GraphAcquisition(acquisition) => match acquisition {
                    BuildSourceAcquisition::Execute(dependency) => {
                        let acquired = dependency.acquired_source().clone();
                        let entry = acquired.entry_path();
                        pending.push(Work::GraphDependency(dependency));
                        pending.push(Work::Source {
                            acquired,
                            entry,
                            execution: Default::default(),
                        });
                    }
                    BuildSourceAcquisition::Reused(next) => pending.push(Work::Graph(next)),
                    BuildSourceAcquisition::Workspace {
                        pending: request,
                        workspace,
                    } => {
                        pending.push(Work::GraphWorkspace(request));
                        pending.push(Work::Acquire(AcquiredSourceSelection::Workspace(workspace)));
                    }
                },
                Work::GraphDependency(dependency) => {
                    let source = take_completed(&mut completed)?.source()?;
                    let (_, next) = dependency
                        .complete_declared(&self.sources[source].declared)
                        .map_err(Source)?;
                    pending.push(Work::Graph(next));
                }
                Work::GraphWorkspace(request) => {
                    let acquired = take_completed(&mut completed)?.acquired()?;
                    pending.push(Work::GraphAcquisition(
                        request.select(acquired).map_err(Source)?,
                    ));
                }
            }
        }
        take_completed(&mut completed)
    }

    fn build_request(
        &mut self,
        acquired: &AcquiredBuildSource,
        entry: &Path,
        execution: package_compilation::BuildExecutionInputs,
    ) -> Result<CheckedCompileRequest, ProjectSourceResolutionError> {
        let mut request = self
            .session
            .packages
            .source_build_request(acquired.source_capture(), entry, self.profile, execution)
            .map_err(Resources)?;
        request.collect_timings = self.timings;
        Ok(request)
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

/// A running guest and its compiler continuation move together across pauses.
struct LiveBuild {
    acquired: AcquiredBuildSource,
    invocation: compiler::PackageBuildInvocation,
    queries: compiler::ProductQueries,
    activation: build_evaluation::TerminalBuildActivation,
    dependencies: Vec<Arc<ResolvedBuildSources>>,
}

enum Demand {
    Provider {
        alias: String,
        declaration: String,
        selected: Option<(usize, usize)>,
    },
    Query {
        query: Arc<build_evaluation::terminal_bindings::BuildProductQuery>,
        dependencies: QueryDependencies,
    },
}

impl Demand {
    fn purpose(&self) -> DependencyPurpose {
        match self {
            Self::Provider { .. } => DependencyPurpose::Build,
            Self::Query { .. } => DependencyPurpose::Product,
        }
    }
}

struct PendingDependencies {
    build: Box<LiveBuild>,
    demand: Demand,
    requests: std::vec::IntoIter<(usize, Option<String>)>,
}

impl PendingDependencies {
    fn new(build: Box<LiveBuild>, demand: Demand) -> Self {
        // Only the cursor and display aliases are copied. Acquisition still
        // consumes the original activation's authenticated request ledger.
        let mut requests = Vec::new();
        let mut ordinal = 0;
        for issued in build.activation.requests() {
            let BuildRequest::Dependency { purpose, alias, .. } = issued.request() else {
                continue;
            };
            if *purpose != demand.purpose() {
                continue;
            }
            let request_ordinal = ordinal;
            ordinal += 1;
            if let Demand::Provider {
                alias: selected, ..
            } = &demand
                && alias
                    .as_ref()
                    .is_some_and(|alias| alias.as_str() != selected)
            {
                continue;
            }
            requests.push((
                request_ordinal,
                alias.as_ref().map(|alias| alias.as_str().to_owned()),
            ));
        }
        Self {
            build,
            demand,
            requests: requests.into_iter(),
        }
    }
}

struct PendingWorkspace {
    members: package_manager::resolution::source::AcquiredWorkspaceMembers,
    next_member: usize,
    declarations: Vec<package_manager::resolution::source::WorkspaceMemberObservation>,
}

// These are application dependency continuations, not compiler-stage jobs.
// The next operation consumes the one completed child; live guest state never
// resides in a callback, recursive call, or serializable checkpoint.
enum Work {
    Source {
        acquired: AcquiredBuildSource,
        entry: std::path::PathBuf,
        execution: package_compilation::BuildExecutionInputs,
    },
    Declare(AcquiredBuildSource),
    Build {
        acquired: AcquiredBuildSource,
        request: CheckedCompileRequest,
    },
    Run(Box<LiveBuild>),
    Dependencies(PendingDependencies),
    DependencySource {
        dependencies: PendingDependencies,
        ordinal: usize,
        alias: Option<String>,
    },
    QueryGraph {
        dependencies: PendingDependencies,
        ordinal: usize,
        source: usize,
        alias: String,
    },
    InstallProvider {
        build: Box<LiveBuild>,
        declaration: String,
        ordinal: usize,
        source: usize,
    },
    Acquire(AcquiredSourceSelection),
    ExecuteAcquired,
    WorkspaceRoot(package_manager::resolution::source::AcquiredWorkspaceBuild),
    WorkspaceMembers(PendingWorkspace),
    WorkspaceMember {
        workspace: PendingWorkspace,
        acquired: AcquiredBuildSource,
    },
    Graph(ExecutedSourceResolution),
    GraphAcquisition(BuildSourceAcquisition),
    GraphDependency(package_manager::resolution::source::AcquiredBuildDependency),
    GraphWorkspace(package_manager::resolution::source::PendingBuildSource),
}

enum CompletedWork {
    Source(usize),
    Build(ExecutedSourceBuild),
    Acquired(AcquiredBuildSource),
    Graph(ResolvedBuildSources),
}

impl CompletedWork {
    fn source(self) -> Result<usize, ProjectSourceResolutionError> {
        match self {
            Self::Source(source) => Ok(source),
            _ => Err(Source("Build scheduler expected a declared source".into())),
        }
    }

    fn build(self) -> Result<ExecutedSourceBuild, ProjectSourceResolutionError> {
        match self {
            Self::Build(build) => Ok(build),
            _ => Err(Source(
                "Build scheduler expected an executed activation".into(),
            )),
        }
    }

    fn acquired(self) -> Result<AcquiredBuildSource, ProjectSourceResolutionError> {
        match self {
            Self::Acquired(source) => Ok(source),
            _ => Err(Source(
                "Build scheduler expected acquired source custody".into(),
            )),
        }
    }

    fn graph(self) -> Result<ResolvedBuildSources, ProjectSourceResolutionError> {
        match self {
            Self::Graph(graph) => Ok(graph),
            _ => Err(Source(
                "Build scheduler expected a resolved source graph".into(),
            )),
        }
    }
}

fn take_completed(
    completed: &mut Option<CompletedWork>,
) -> Result<CompletedWork, ProjectSourceResolutionError> {
    completed
        .take()
        .ok_or_else(|| Source("Build scheduler lost its completed dependency".into()))
}
