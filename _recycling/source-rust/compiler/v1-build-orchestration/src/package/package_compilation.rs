/*

Compile admitted package plans across targets (`compile_packages`, `prepare_package`).

We own the dependency-first frontier scheduling (`prepare_packages`),
generated-source handoffs, and independent-component barriers across all
requested targets, while `CompilationSession` owns source checking, settlement,
and Terminal Psi production:
- `compile_packages` advances every target's `PackagePlan` in lockstep on a
  single compiler worker thread (`run_on_compile_thread`), batch-completes the
  prepared root products (`compiler.complete_package_products`), and finishes any
  native artifacts through `native_completion::complete_products`. Settled
  occurrences must already exist in `completed`; a missing entry fails closed
  rather than re-executing `build.omg`.
- `prepare_package` runs the dependency-first source preparation for a single
  package plan without checking its final Product, returning the opaque
  `PackageBuild` custody.

*/

use crate::compile::build_execution::{PackagePlan, PreparedPackageRoot, prepare_packages};
use compiler::{
    CompilationOutput, CompilationSession, PackageBuild, PackageCompilationRequest,
    PackageCompileFailure, PackageSourceRequest, PreparedPackageProduct,
};
use diagnostics::Diagnostic;

pub fn compile_packages(
    session: &mut crate::BuildSession,
    requests: Vec<PackageCompilationRequest>,
    completed: &mut [compiler::CompletedBuild],
) -> Vec<Result<CompilationOutput, PackageCompileFailure>> {
    let crate::BuildSession {
        compiler,
        packages: resources,
    } = session;
    let mut plans = Vec::with_capacity(requests.len());
    for (position, mut request) in requests.into_iter().enumerate() {
        let root = request.occurrences.len().saturating_sub(1);
        for occurrence in &mut request.occurrences {
            occurrence.set_optimization_rollback(request.product.optimization_rollback());
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
                occurrences: request.occurrences,
            },
        ));
    }

    let count = plans.len();
    compiler::checked::run_on_compile_thread(|| {
        let mut outcomes = Vec::with_capacity(count);
        let mut products = Vec::new();
        for ((position, root, product, permissions), prepared) in
            prepare_packages(compiler, plans, completed)
        {
            match prepared {
                Ok(PreparedPackageRoot::Build(build)) => {
                    resources
                        .record_source_completion(build.source_completion_accounting().clone());
                    products.push((
                        (position, root),
                        PreparedPackageProduct {
                            build: *build,
                            product,
                            permissions,
                        },
                    ));
                }
                Err(error) => {
                    if let Some(accounting) = error.failure.source_completion_accounting() {
                        resources.record_source_completion(accounting.clone());
                    }
                    outcomes.push((position, Err(error)));
                }
            }
        }

        let products = compiler.complete_package_products(products)?;
        for ((position, root), product) in
            crate::compile::native_completion::complete_products(products)
        {
            outcomes.push((
                position,
                product.map_err(|error| PackageCompileFailure {
                    occurrence: root,
                    failure: error.into(),
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

pub fn prepare_package(
    compiler: &mut CompilationSession,
    occurrences: Vec<PackageSourceRequest>,
    completed: &mut [compiler::CompletedBuild],
) -> Result<PackageBuild, PackageCompileFailure> {
    let root = occurrences.len().saturating_sub(1);
    compiler::checked::run_on_compile_thread(|| {
        let prepared =
            prepare_packages(compiler, vec![((), PackagePlan { occurrences })], completed).pop();
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
