//! Advance package occurrences dependency-first across all requested products.
//!
//! Each occurrence admits its own Build and keeps its generated-byte custody.
//! Independent components demanded at the same frontier enter source completion
//! together. A target roster no longer completes its whole chain before sibling
//! targets can contribute the same dependency source.
//!
//! A frontier either prepares its Product root's Build or, for a caller that
//! compiles that root through its own product request, stops after joining the
//! dependencies' generated source into the root request.

use super::{CompilationSession, PackageCompileFailure};
use build_declarations::DependencyPurpose;
use compilation_products::{CheckedCompileRequest, CompileFailure, PreparedPackageBuild};
use diagnostics::Diagnostic;
use package_compilation::{IndependentComponentDescription, PackageGeneratedSourceBundle};
use semantic_vocabulary::PackageKeyIdentity;

struct OccurrenceProduct {
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    generated: PackageGeneratedSourceBundle,
    prepared: Option<PreparedPackageBuild>,
    description: Option<IndependentComponentDescription>,
}

struct PackageFrontier<Owner> {
    position: usize,
    owner: Owner,
    remaining: std::vec::IntoIter<CheckedCompileRequest>,
    products: Vec<OccurrenceProduct>,
    /// The Product root a joining frontier receives its dependencies'
    /// generated source into instead of preparing; `None` prepares the last
    /// remaining occurrence as the root.
    joined_root: Option<CheckedCompileRequest>,
}

/// How one package plan finished: its root's prepared Build, or the root
/// request joined with every dependency occurrence's generated source.
pub(super) enum PreparedPackageRoot {
    Build(Box<PreparedPackageBuild>),
    Joined(CheckedCompileRequest),
}

/// One dependency-ordered plan and what to do with its Product root.
pub(super) struct PackagePlan {
    pub occurrences: Vec<CheckedCompileRequest>,
    /// Join generated source into the root instead of preparing its Build.
    pub join_root: bool,
}

struct PendingOccurrence<Owner> {
    frontier: PackageFrontier<Owner>,
    build: PreparedPackageBuild,
    components: Vec<usize>,
    failure: Option<CompileFailure>,
}

impl CompilationSession {
    pub(super) fn prepare_packages<Owner>(
        &mut self,
        plans: Vec<(Owner, PackagePlan)>,
    ) -> Vec<(Owner, Result<PreparedPackageRoot, PackageCompileFailure>)> {
        let mut outcomes = Vec::with_capacity(plans.len());
        let mut frontiers = Vec::new();
        for (position, (owner, plan)) in plans.into_iter().enumerate() {
            let PackagePlan {
                mut occurrences,
                join_root,
            } = plan;
            if let Err(error) = validate_order(&occurrences) {
                outcomes.push((position, owner, Err(error)));
                continue;
            }
            let joined_root = if join_root { occurrences.pop() } else { None };
            let mut frontier = PackageFrontier {
                position,
                owner,
                products: Vec::with_capacity(occurrences.len()),
                remaining: occurrences.into_iter(),
                joined_root,
            };
            if frontier.remaining.len() == 0 {
                // A joining root with no dependency occurrence joins the
                // empty roster; a preparing root always has its own.
                let joined = frontier.join_root();
                outcomes.push((position, frontier.owner, joined));
                continue;
            }
            frontiers.push(frontier);
        }

        while !frontiers.is_empty() {
            let mut pending = Vec::with_capacity(frontiers.len());
            for mut frontier in frontiers {
                let prepared = (|| {
                    let occurrence = frontier.remaining.next().ok_or_else(|| {
                        vec![Diagnostic::error(
                            "package frontier lost its next occurrence",
                        )]
                    })?;
                    let occurrence = join_generated_sources(occurrence, &frontier.products)?;
                    let build = self.prepare_build(occurrence)?;
                    let components = required_components(&build, &frontier.products)?;
                    Ok::<_, CompileFailure>((build, components))
                })();
                match prepared {
                    Ok((build, components)) => pending.push(PendingOccurrence {
                        frontier,
                        build,
                        components,
                        failure: None,
                    }),
                    Err(failure) => {
                        outcomes.push((
                            frontier.position,
                            frontier.owner,
                            Err(PackageCompileFailure {
                                occurrence: frontier.products.len(),
                                failure,
                            }),
                        ));
                    }
                }
            }

            self.complete_components(&mut pending);

            frontiers = Vec::with_capacity(pending.len());
            for pending in pending {
                let PendingOccurrence {
                    mut frontier,
                    build,
                    components,
                    failure,
                } = pending;
                let completed = if let Some(failure) = failure {
                    Err(failure)
                } else {
                    let descriptions = components
                        .into_iter()
                        .map(|position| {
                            frontier.products[position]
                                .description
                                .clone()
                                .ok_or_else(|| {
                                    vec![Diagnostic::error(
                                        "component completion lost its description",
                                    )]
                                })
                        })
                        .collect::<Result<Vec<_>, _>>();
                    descriptions
                        .map_err(CompileFailure::from)
                        .and_then(|descriptions| {
                            build
                                .with_independent_component_descriptions(descriptions)
                                .map_err(|errors| CompileFailure::from(input_diagnostics(errors)))
                        })
                };
                match completed {
                    Err(failure) => {
                        outcomes.push((
                            frontier.position,
                            frontier.owner,
                            Err(PackageCompileFailure {
                                occurrence: frontier.products.len(),
                                failure,
                            }),
                        ));
                    }
                    Ok(build)
                        if frontier.remaining.len() == 0 && frontier.joined_root.is_none() =>
                    {
                        outcomes.push((
                            frontier.position,
                            frontier.owner,
                            Ok(PreparedPackageRoot::Build(Box::new(build))),
                        ));
                    }
                    Ok(build) => {
                        frontier.products.push(OccurrenceProduct {
                            package: build.package_inputs().root(),
                            purpose: build.package_inputs().compilation_purpose(),
                            generated: build.generated_source_bundle().clone(),
                            prepared: Some(build),
                            description: None,
                        });
                        if frontier.remaining.len() == 0 {
                            let joined = frontier.join_root();
                            outcomes.push((frontier.position, frontier.owner, joined));
                        } else {
                            frontiers.push(frontier);
                        }
                    }
                }
            }
        }

        outcomes.sort_unstable_by_key(|(position, _, _)| *position);
        outcomes
            .into_iter()
            .map(|(_, owner, outcome)| (owner, outcome))
            .collect()
    }

    /// Complete actual component demands as one source cohort batch, then retain
    /// each target's own installed description. Descriptions are not shared
    /// authority, even when their source arenas are common.
    fn complete_components<Owner>(&mut self, pending: &mut [PendingOccurrence<Owner>]) {
        let mut builds = Vec::new();
        for (request_position, occurrence) in pending.iter_mut().enumerate() {
            for &component_position in &occurrence.components {
                let product = &mut occurrence.frontier.products[component_position];
                if product.description.is_some() {
                    continue;
                }
                let Some(build) = product.prepared.take() else {
                    occurrence.failure = Some(
                        vec![Diagnostic::error(
                            "independent component continuation was already consumed",
                        )]
                        .into(),
                    );
                    break;
                };
                builds.push(((request_position, component_position), build));
            }
        }
        let checked = self
            .select_build_batch(builds)
            .into_iter()
            .map(|(destination, selected)| {
                (
                    destination,
                    selected
                        .install()
                        .result
                        .map_err(CompileFailure::into_diagnostics),
                )
            })
            .collect();
        for ((request_position, component_position), product) in
            crate::compiler::package::publish_checked_component_products(checked)
        {
            let occurrence = &mut pending[request_position];
            match product {
                Ok(product) => {
                    occurrence.frontier.products[component_position].description =
                        Some(product.description().clone());
                    self.retain_component_product(product);
                }
                Err(failure) => {
                    if occurrence.failure.is_none() {
                        occurrence.failure = Some(failure.into());
                    }
                }
            }
        }
    }
}

impl<Owner> PackageFrontier<Owner> {
    /// Finish a joining frontier: its root receives every dependency
    /// occurrence's generated source and is returned unprepared.
    fn join_root(&mut self) -> Result<PreparedPackageRoot, PackageCompileFailure> {
        let occurrence = self.products.len();
        let refuse = |failure| PackageCompileFailure {
            occurrence,
            failure,
        };
        let root = self.joined_root.take().ok_or_else(|| {
            refuse(vec![Diagnostic::error("package frontier lost its joined root")].into())
        })?;
        join_generated_sources(root, &self.products)
            .map(PreparedPackageRoot::Joined)
            .map_err(refuse)
    }
}

fn join_generated_sources(
    mut occurrence: CheckedCompileRequest,
    products: &[OccurrenceProduct],
) -> Result<CheckedCompileRequest, CompileFailure> {
    let inputs = occurrence.package_inputs.take().ok_or_else(|| {
        vec![Diagnostic::error(
            "package compilation requires source custody",
        )]
    })?;
    let required = inputs.required_dependency_source_instances();
    let mut generated = Vec::with_capacity(required.len());
    for (dependency, dependency_purpose) in required {
        let product = products
            .iter()
            .find(|product| product.package == dependency && product.purpose == dependency_purpose)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "dependency Build output is missing for its required purpose",
                )]
            })?;
        generated.push(product.generated.clone());
    }
    occurrence.package_inputs = Some(
        inputs
            .with_complete_dependency_generated_sources(generated)
            .map_err(input_diagnostics)?,
    );
    Ok(occurrence)
}

fn required_components(
    prepared: &PreparedPackageBuild,
    products: &[OccurrenceProduct],
) -> Result<Vec<usize>, CompileFailure> {
    let inputs = prepared.package_inputs();
    let mut components = Vec::new();
    for selection in prepared.build().independent_components().selections {
        let Some(component) = selection.component else {
            continue;
        };
        if component == inputs.root() {
            continue;
        }
        let position = products
            .iter()
            .position(|product| {
                product.package == component && product.purpose == inputs.compilation_purpose()
            })
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "independent component has no prepared Build for this purpose",
                )]
            })?;
        if !components.contains(&position) {
            components.push(position);
        }
    }
    Ok(components)
}

/// Refuse malformed plans before any Build effect. Order is supplied by the
/// package resolver; checking it is request admission, not graph reconstruction.
fn validate_order(request: &[CheckedCompileRequest]) -> Result<(), PackageCompileFailure> {
    if request.is_empty() {
        return Err(PackageCompileFailure {
            occurrence: 0,
            failure: vec![Diagnostic::error("package compilation has no Product root")].into(),
        });
    }
    for (position, occurrence) in request.iter().enumerate() {
        let refuse = |message| PackageCompileFailure {
            occurrence: position,
            failure: vec![Diagnostic::error(message)].into(),
        };
        let inputs = occurrence
            .package_inputs
            .as_ref()
            .ok_or_else(|| refuse("package compilation requires source custody"))?;
        let previous = &request[..position];
        if previous
            .iter()
            .filter_map(|request| request.package_inputs.as_ref())
            .any(|prior| {
                prior.root() == inputs.root()
                    && prior.compilation_purpose() == inputs.compilation_purpose()
            })
        {
            return Err(refuse("package compilation repeats a package occurrence"));
        }
        for (package, purpose) in inputs.required_dependency_source_instances() {
            if !previous
                .iter()
                .filter_map(|request| request.package_inputs.as_ref())
                .any(|prior| prior.root() == package && prior.compilation_purpose() == purpose)
            {
                return Err(refuse(
                    "package compilation requires dependency-first occurrence order",
                ));
            }
        }
        if position + 1 == request.len() && !inputs.compilation_purpose().is_product() {
            return Err(refuse("package compilation must end at its Product root"));
        }
    }
    Ok(())
}

fn input_diagnostics(
    errors: Vec<package_compilation::PackageCompilationInputError>,
) -> Vec<Diagnostic> {
    errors
        .into_iter()
        .map(|error| Diagnostic::error(error.to_string()))
        .collect()
}
