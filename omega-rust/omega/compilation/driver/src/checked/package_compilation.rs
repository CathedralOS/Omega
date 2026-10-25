//! Complete individual Build activations and their requested source products.
//!
//! Application Build owns dependency order and generated-source joins. This
//! facade keeps source continuations, source cohort checking, component custody,
//! provider settlement and Terminal/native production inside the compiler.

use super::CompilationSession;
use compilation_products::{CheckedCompileRequest, CompileFailure, PreparedPackageBuild};
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

/// The occurrence responsible for a rejected package request.
pub struct PackageCompileFailure {
    pub occurrence: usize,
    pub failure: CompileFailure,
}

/// A completed Build with compiler-private source continuation. Applications
/// can schedule its generated sources and demanded packages, but cannot extract
/// typed trees, alter source completion, or install a checked checkpoint.
pub struct PackageBuild {
    source: PreparedPackageBuild,
}

impl PackageBuild {
    pub fn package_inputs(&self) -> &package_compilation::PackageCompilationInputs {
        self.source.package_inputs()
    }

    pub fn generated_source_bundle(&self) -> &package_compilation::PackageGeneratedSourceBundle {
        self.source.generated_source_bundle()
    }

    pub fn required_component_packages(&self) -> Vec<semantic_vocabulary::PackageKeyIdentity> {
        self.source
            .build()
            .independent_components()
            .selections
            .into_iter()
            .filter_map(|selection| selection.component)
            .collect()
    }

    /// Only descriptions may join after Build; source, purpose, input custody
    /// and grants remain the ones this activation actually consumed.
    pub fn with_independent_component_descriptions(
        self,
        descriptions: Vec<package_compilation::IndependentComponentDescription>,
    ) -> Result<Self, Vec<package_compilation::PackageCompilationInputError>> {
        self.source
            .with_independent_component_descriptions(descriptions)
            .map(|source| Self { source })
    }
}

/// A completed Build and the requested product of its retained source.
/// Generated inputs and demanded component descriptions have already joined
/// this exact activation; completing it does not execute Build again.
pub struct PreparedPackageProduct {
    pub build: PackageBuild,
    pub product: crate::CompilationProduct,
    pub permissions: Option<package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
}

impl CompilationSession {
    /// Execute one admitted occurrence's Build, retaining opaque Product work.
    /// The caller may join its generated output into dependent Build requests.
    pub fn prepare_package_build(
        &mut self,
        request: CheckedCompileRequest,
    ) -> Result<PackageBuild, CompileFailure> {
        super::run_on_compile_thread(|| {
            self.prepare_build(request)
                .map(|source| PackageBuild { source })
        })
    }

    /// Check a completed Build's source for compiler/diagnostic consumers.
    /// Package review requests review_package_build instead of checked custody;
    /// application Build submits products through complete_package_products.
    pub fn check_package_build(
        &mut self,
        build: PackageBuild,
    ) -> Result<super::CheckedCompilation, CompileFailure> {
        super::run_on_compile_thread(|| {
            let ((), selected) = self
                .select_build_batch(vec![((), build.source)])
                .pop()
                .ok_or_else(|| {
                    vec![Diagnostic::error("package checking lost its selected root")]
                })?;
            selected.install().result
        })
    }

    /// Complete demanded independent components as one source cohort batch.
    /// Retain executable custody in this session and return only descriptions
    /// for the caller to join to waiting consumers. A description alone never
    /// grants a consumer executable authority.
    pub fn complete_package_components<Owner: Send>(
        &mut self,
        builds: Vec<(Owner, PackageBuild)>,
    ) -> Result<
        Vec<(
            Owner,
            Result<package_compilation::IndependentComponentDescription, Vec<Diagnostic>>,
        )>,
        Vec<Diagnostic>,
    > {
        super::run_on_compile_thread(|| {
            let builds = builds
                .into_iter()
                .map(|(owner, build)| (owner, build.source))
                .collect();
            let checked = self
                .select_build_batch(builds)
                .into_iter()
                .map(|(owner, selected)| {
                    (
                        owner,
                        selected
                            .install()
                            .result
                            .map_err(CompileFailure::into_diagnostics),
                    )
                })
                .collect();
            let mut completed = Vec::new();
            for (owner, product) in
                crate::compiler::package::publish_checked_component_products(checked)
            {
                let description = match product {
                    Ok(product) => {
                        let description = product.description().clone();
                        self.retain_component_product(product);
                        Ok(description)
                    }
                    Err(diagnostics) => Err(diagnostics),
                };
                completed.push((owner, description));
            }
            Ok(completed)
        })
    }

    /// Complete ready roots together, sharing source/Terminal work while each
    /// request retains its own policy, permission consent and result.
    pub fn complete_package_products<Owner: Send>(
        &mut self,
        requests: Vec<(Owner, PreparedPackageProduct)>,
    ) -> Result<
        Vec<(
            Owner,
            Result<crate::CompilationOutput, crate::CompilationProductError>,
        )>,
        Vec<Diagnostic>,
    > {
        super::run_on_compile_thread(|| {
            let builds = requests
                .into_iter()
                .map(|(owner, request)| {
                    let root = request.build.source.root_path().to_path_buf();
                    (
                        (owner, root, request.product, request.permissions),
                        request.build.source,
                    )
                })
                .collect();
            let mut products = Vec::new();
            for ((owner, root, product, permissions), selected) in self.select_build_batch(builds) {
                let request = selected
                    .result
                    .map(|realization| crate::product_execution::ProductRequest {
                        realization,
                        root,
                        product,
                        permissions,
                    })
                    .map_err(|failure| {
                        crate::CompilationProductError::Production(failure.into_diagnostics())
                    });
                products.push((owner, request));
            }
            Ok(crate::compiler::produce_products(products))
        })
    }
}
