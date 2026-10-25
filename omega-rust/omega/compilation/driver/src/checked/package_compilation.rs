//! Execute a dependency-ordered package request through the compiler session.
//!
//! Package management supplies immutable source custody, accepted bindings and
//! occurrence-specific consent. This owner joins generated outputs, retains
//! dependency continuations, and completes only demanded independent products
//! before checking the root. Package callers do not assemble compiler stages.

use super::CompilationSession;
use super::package_preparation::{PackagePlan, PreparedPackageRoot};
use compilation_products::{CheckedCompileRequest, CompileFailure};
use diagnostics::Diagnostic;

/// One dependency-ordered source closure. The last request is its Product root;
/// earlier requests are dependency occurrences, including separate Build roles.
pub struct PackageCompilationRequest {
    pub occurrences: Vec<CheckedCompileRequest>,
    pub product: crate::CompilationProduct,
    /// Present only for a locked root Product. No lock supplies no permissions;
    /// a missing occurrence inside an existing lock must reject at admission.
    pub terminal_permissions:
        Option<package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
}

/// The occurrence responsible for a rejected compiler request. Package naming
/// and presentation remain with the package owner.
pub struct PackageCompileFailure {
    pub occurrence: usize,
    pub failure: CompileFailure,
}

/// Caller identity and product policy stay attached while package frontiers
/// and source cohorts complete in a different order.
struct PackageProductOwner {
    position: usize,
    root_occurrence: usize,
    root_path: std::path::PathBuf,
    product: crate::CompilationProduct,
    permissions: Option<package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
}

impl CompilationSession {
    /// Check a dependency-ordered package activation without producing its root.
    /// Demanded independent dependencies still require their own verified products;
    /// a description alone cannot settle the root's boundary applications.
    pub fn check_package(
        &mut self,
        occurrences: Vec<CheckedCompileRequest>,
    ) -> Result<super::CheckedCompilation, PackageCompileFailure> {
        let root = occurrences.len().saturating_sub(1);
        super::run_on_compile_thread(|| {
            let prepared = self
                .prepare_packages(vec![(
                    (),
                    PackagePlan {
                        occurrences,
                        join_root: false,
                    },
                )])
                .pop();
            let build = match prepared {
                Some(((), Ok(PreparedPackageRoot::Build(build)))) => *build,
                Some(((), Err(error))) => return Ok(Err(error)),
                _ => {
                    return Ok(Err(PackageCompileFailure {
                        occurrence: root,
                        failure: vec![Diagnostic::error("package checking lost its prepared root")]
                            .into(),
                    }));
                }
            };
            let ((), selected) = self
                .select_build_batch(vec![((), build)])
                .pop()
                .ok_or_else(|| {
                    vec![Diagnostic::error("package checking lost its selected root")]
                })?;
            Ok::<_, Vec<Diagnostic>>(selected.install().result.map_err(|failure| {
                PackageCompileFailure {
                    occurrence: root,
                    failure,
                }
            }))
        })
        .unwrap_or_else(|diagnostics| {
            Err(PackageCompileFailure {
                occurrence: root,
                failure: diagnostics.into(),
            })
        })
    }

    /// Compile the complete admitted roster through its requested products.
    /// Source checking, consent comparison and production belong to this call;
    /// package operations provide policy inputs, not a mid-pipeline callback.
    pub fn compile_packages(
        &mut self,
        requests: Vec<PackageCompilationRequest>,
    ) -> Vec<Result<crate::CompilationOutput, PackageCompileFailure>> {
        let count = requests.len();
        super::run_on_compile_thread(|| {
            let mut outcomes = Vec::with_capacity(requests.len());
            let mut plans = Vec::with_capacity(requests.len());
            for (position, mut request) in requests.into_iter().enumerate() {
                let root = request.occurrences.len().saturating_sub(1);
                let path = request
                    .occurrences
                    .last()
                    .map(|root| root.root_path.clone())
                    .unwrap_or_default();
                for occurrence in &mut request.occurrences {
                    occurrence.optimization_rollback = request.product.optimization_rollback();
                }
                let owner = PackageProductOwner {
                    position,
                    root_occurrence: root,
                    root_path: path,
                    product: request.product,
                    permissions: request.terminal_permissions,
                };
                plans.push((
                    owner,
                    PackagePlan {
                        occurrences: request.occurrences,
                        join_root: false,
                    },
                ));
            }

            let mut builds = Vec::new();
            for (owner, prepared) in self.prepare_packages(plans) {
                match prepared {
                    Ok(PreparedPackageRoot::Build(build)) => {
                        builds.push((owner, *build));
                    }
                    Ok(PreparedPackageRoot::Joined(_)) => {
                        outcomes.push((
                            owner.position,
                            Err(PackageCompileFailure {
                                occurrence: owner.root_occurrence,
                                failure: vec![Diagnostic::error(
                                    "package compilation joined a root it was asked to prepare",
                                )]
                                .into(),
                            }),
                        ));
                    }
                    Err(error) => outcomes.push((owner.position, Err(error))),
                }
            }

            // All roots enter source completion together. Provider selection feeds
            // the same Terminal-before-installation sequence as standalone builds.
            // Permission consent travels to that sequence's admission boundary.
            let mut products = Vec::new();
            for (owner, selected) in self.select_build_batch(builds) {
                match selected.result {
                    Ok(selected) => {
                        products.push((
                            (owner.position, owner.root_occurrence),
                            Ok(crate::product_execution::ProductRequest {
                                realization: selected,
                                root: owner.root_path,
                                product: owner.product,
                                permissions: owner.permissions,
                            }),
                        ));
                    }
                    Err(failure) => {
                        outcomes.push((
                            owner.position,
                            Err(PackageCompileFailure {
                                occurrence: owner.root_occurrence,
                                failure,
                            }),
                        ));
                    }
                }
            }
            for ((position, root), product) in crate::compiler::produce_products(products) {
                outcomes.push((
                    position,
                    product.map_err(|error| PackageCompileFailure {
                        occurrence: root,
                        failure: error.into_diagnostics().into(),
                    }),
                ));
            }
            outcomes.sort_unstable_by_key(|(position, _)| *position);
            Ok::<_, Vec<Diagnostic>>(outcomes.into_iter().map(|(_, outcome)| outcome).collect())
        })
        .unwrap_or_else(|diagnostics| {
            (0..count)
                .map(|_| {
                    Err(PackageCompileFailure {
                        occurrence: 0,
                        failure: diagnostics.clone().into(),
                    })
                })
                .collect()
        })
    }

    /// Execute every dependency occurrence of one dependency-ordered plan, as
    /// [`Self::compile_packages`] does, and return its Product root's package
    /// inputs joined with their complete generated source, without preparing
    /// or checking the root. `occurrences` has the shape of
    /// [`PackageCompilationRequest::occurrences`]: the Product root is last.
    ///
    /// The corpus runner's fixture harness calls this. Each of its legs
    /// compiles a fixture root through its own product request, so the root
    /// needs dependency Build output joined into its inputs rather than a
    /// finished package product. Package operations submit whole plans
    /// through [`Self::compile_packages`] instead.
    pub fn join_dependency_builds(
        &mut self,
        occurrences: Vec<CheckedCompileRequest>,
    ) -> Result<package_compilation::PackageCompilationInputs, PackageCompileFailure> {
        let root = occurrences.len().saturating_sub(1);
        super::run_on_compile_thread(|| {
            let prepared = self
                .prepare_packages(vec![(
                    (),
                    PackagePlan {
                        occurrences,
                        join_root: true,
                    },
                )])
                .pop();
            Ok::<_, Vec<Diagnostic>>(match prepared {
                Some(((), Ok(PreparedPackageRoot::Joined(request)))) => {
                    request.package_inputs.ok_or_else(|| PackageCompileFailure {
                        occurrence: root,
                        failure: vec![Diagnostic::error(
                            "package compilation requires source custody",
                        )]
                        .into(),
                    })
                }
                Some(((), Err(error))) => Err(error),
                Some(((), Ok(PreparedPackageRoot::Build(_)))) | None => {
                    Err(PackageCompileFailure {
                        occurrence: root,
                        failure: vec![Diagnostic::error(
                            "package preparation lost its joined root",
                        )]
                        .into(),
                    })
                }
            })
        })
        .unwrap_or_else(|diagnostics| {
            Err(PackageCompileFailure {
                occurrence: 0,
                failure: diagnostics.into(),
            })
        })
    }
}
