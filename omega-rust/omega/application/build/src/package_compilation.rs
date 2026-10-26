//! Schedule admitted package Builds, then complete ready products together.
//!
//! The application owns dependency order, generated-source joins and which
//! components must finish before a consumer can proceed. The compiler retains
//! opaque source continuations and owns checking, settlement and production.

use crate::build_execution::{
    PackagePlan, PackageSourceRequest, PreparedPackageRoot, prepare_packages,
};
use compiler::{
    CheckedCompileRequest, CompilationOutput, CompilationSession, PackageBuild,
    PackageCompilationRequest, PackageCompileFailure, PreparedPackageProduct,
};
use diagnostics::Diagnostic;

/// Execute a dependency-ordered roster and finish each requested product.
/// The whole invocation uses one compiler-stack worker; compiler operations
/// reuse that worker rather than spawning one per dependency occurrence.
pub fn compile_packages(
    compiler: &mut CompilationSession,
    requests: Vec<PackageCompilationRequest>,
) -> Vec<Result<CompilationOutput, PackageCompileFailure>> {
    let mut plans = Vec::with_capacity(requests.len());
    for (position, mut request) in requests.into_iter().enumerate() {
        let root = request.occurrences.len().saturating_sub(1);
        for occurrence in &mut request.occurrences {
            occurrence.optimization_rollback = request.product.optimization_rollback();
        }
        let owner = (
            position,
            root,
            request.product,
            request.terminal_permissions,
        );
        plans.push((
            owner,
            PackagePlan {
                occurrences: request
                    .occurrences
                    .into_iter()
                    .map(PackageSourceRequest::Execute)
                    .collect(),
                join_root: false,
            },
        ));
    }

    compile_plans(compiler, plans, &mut [])
}

pub(crate) fn compile_returned_packages(
    compiler: &mut CompilationSession,
    requests: Vec<package_manager::operations::PackageProductCompilationRequest>,
    completed: &mut [compiler::CompletedBuild],
) -> Vec<Result<CompilationOutput, PackageCompileFailure>> {
    let mut plans = Vec::with_capacity(requests.len());
    for (position, request) in requests.into_iter().enumerate() {
        let root = request.occurrences.len().saturating_sub(1);
        plans.push((
            (
                position,
                root,
                request.product,
                request.terminal_permissions,
            ),
            PackagePlan {
                occurrences: request
                    .occurrences
                    .into_iter()
                    .map(PackageSourceRequest::Product)
                    .collect(),
                join_root: false,
            },
        ));
    }
    compile_plans(compiler, plans, completed)
}

type RequestedPackageProduct = (
    usize,
    usize,
    compiler::CompilationProduct,
    Option<compiler::TerminalPermissionAcceptance>,
);

fn compile_plans(
    compiler: &mut CompilationSession,
    plans: Vec<(RequestedPackageProduct, PackagePlan)>,
    completed: &mut [compiler::CompletedBuild],
) -> Vec<Result<CompilationOutput, PackageCompileFailure>> {
    let count = plans.len();
    compiler::checked::run_on_compile_thread(|| {
        let mut outcomes = Vec::with_capacity(count);
        let mut products = Vec::new();
        for ((position, root, product, permissions), prepared) in
            prepare_packages(compiler, plans, completed)
        {
            match prepared {
                Ok(PreparedPackageRoot::Build(build)) => products.push((
                    (position, root),
                    PreparedPackageProduct {
                        build: *build,
                        product,
                        permissions,
                    },
                )),
                Ok(PreparedPackageRoot::Joined(_)) => outcomes.push((
                    position,
                    Err(PackageCompileFailure {
                        occurrence: root,
                        failure: vec![Diagnostic::error(
                            "package compilation joined a root it was asked to prepare",
                        )]
                        .into(),
                    }),
                )),
                Err(error) => outcomes.push((position, Err(error))),
            }
        }

        for ((position, root), product) in compiler.complete_package_products(products)? {
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

/// Execute a package's dependency Builds and root Build, without checking its
/// Product source. The returned continuation is opaque compiler custody, not
/// an application-accessible source representation or permission to publish.
pub fn prepare_package(
    compiler: &mut CompilationSession,
    occurrences: Vec<CheckedCompileRequest>,
) -> Result<PackageBuild, PackageCompileFailure> {
    let root = occurrences.len().saturating_sub(1);
    compiler::checked::run_on_compile_thread(|| {
        let prepared = prepare_packages(
            compiler,
            vec![(
                (),
                PackagePlan {
                    occurrences: occurrences
                        .into_iter()
                        .map(PackageSourceRequest::Execute)
                        .collect(),
                    join_root: false,
                },
            )],
            &mut [],
        )
        .pop();
        Ok::<_, Vec<Diagnostic>>(match prepared {
            Some(((), Ok(PreparedPackageRoot::Build(build)))) => Ok(*build),
            Some(((), Err(error))) => Err(error),
            _ => Err(PackageCompileFailure {
                occurrence: root,
                failure: vec![Diagnostic::error("package checking lost its prepared root")].into(),
            }),
        })
    })
    .unwrap_or_else(|diagnostics| {
        Err(PackageCompileFailure {
            occurrence: root,
            failure: diagnostics.into(),
        })
    })
}

/// Execute dependencies and join their generated source into the root's inputs,
/// leaving the root Build unexecuted. Diagnostic/review consumers may then
/// compile that root under their own requested stop through the same compiler.
pub fn join_dependency_builds(
    compiler: &mut CompilationSession,
    occurrences: Vec<CheckedCompileRequest>,
) -> Result<package_compilation::PackageCompilationInputs, PackageCompileFailure> {
    let root = occurrences.len().saturating_sub(1);
    compiler::checked::run_on_compile_thread(|| {
        let prepared = prepare_packages(
            compiler,
            vec![(
                (),
                PackagePlan {
                    occurrences: occurrences
                        .into_iter()
                        .map(PackageSourceRequest::Execute)
                        .collect(),
                    join_root: true,
                },
            )],
            &mut [],
        )
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
            _ => Err(PackageCompileFailure {
                occurrence: root,
                failure: vec![Diagnostic::error(
                    "package preparation lost its joined root",
                )]
                .into(),
            }),
        })
    })
    .unwrap_or_else(|diagnostics| {
        Err(PackageCompileFailure {
            occurrence: 0,
            failure: diagnostics.into(),
        })
    })
}
