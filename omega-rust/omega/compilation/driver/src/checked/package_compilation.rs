//! Execute a dependency-ordered package request through the compiler session.
//!
//! Package management supplies immutable source custody, accepted bindings and
//! occurrence-specific consent. This owner joins generated outputs, retains
//! dependency continuations, and completes only demanded independent products
//! before checking the root. Package callers do not assemble compiler stages.

use super::CompilationSession;
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

impl CompilationSession {
    /// Compile the complete admitted roster through its requested products.
    /// Source checking, consent comparison and production belong to this call;
    /// package operations provide policy inputs, not a mid-pipeline callback.
    pub fn compile_packages(
        &mut self,
        requests: Vec<PackageCompilationRequest>,
    ) -> Vec<Result<crate::CompilationOutput, PackageCompileFailure>> {
        let count = requests.len();
        super::run_on_compile_thread(|| {
            let mut outcomes: Vec<_> = (0..requests.len()).map(|_| None).collect();
            let mut selections = Vec::new();
            let mut plans = Vec::new();
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
                selections.push((
                    position,
                    root,
                    path,
                    request.product,
                    request.terminal_permissions,
                ));
                plans.push(request.occurrences);
            }

            let mut builds = Vec::new();
            let mut admitted = Vec::new();
            for (prepared, selection) in self.prepare_packages(plans).into_iter().zip(selections) {
                match prepared {
                    Ok(build) => {
                        builds.push(build);
                        admitted.push(selection);
                    }
                    Err(error) => outcomes[selection.0] = Some(Err(error)),
                }
            }

            // All roots enter source completion together. Provider selection feeds
            // the same Terminal-before-installation sequence as standalone builds.
            // Permission consent travels to that sequence's admission boundary.
            let mut positions = Vec::new();
            let mut products = Vec::new();
            for (selected, (position, root, path, product, permissions)) in
                self.select_build_batch(builds).into_iter().zip(admitted)
            {
                match selected.result {
                    Ok(selected) => {
                        positions.push((position, root));
                        products.push(Ok((selected, path, product, permissions)));
                    }
                    Err(failure) => {
                        outcomes[position] = Some(Err(PackageCompileFailure {
                            occurrence: root,
                            failure,
                        }))
                    }
                }
            }
            for ((position, root), product) in positions
                .into_iter()
                .zip(crate::compiler::finish_compilations(products))
            {
                outcomes[position] = Some(product.map_err(|error| PackageCompileFailure {
                    occurrence: root,
                    failure: error.into_diagnostics().into(),
                }));
            }
            Ok::<_, Vec<Diagnostic>>(
                outcomes
                    .into_iter()
                    .map(|outcome| {
                        outcome.unwrap_or_else(|| {
                            Err(PackageCompileFailure {
                                occurrence: 0,
                                failure: vec![Diagnostic::error(
                                    "package compilation lost a requested product",
                                )]
                                .into(),
                            })
                        })
                    })
                    .collect(),
            )
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
}
