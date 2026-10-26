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

/// An admitted Build with opaque pending Product source. It owns its code and
/// grants independently of the compiler session, so dependency work can proceed
/// while the caller keeps this invocation suspended.
pub struct PackageBuildInvocation {
    source: super::checking::PreparedPackageInvocation,
}

/// Settled Build effects and captured source, before Product preparation.
/// Independent Product admissions borrow this output; none can replay Build or
/// replace its source graph, execution inputs, output bytes or observations.
pub struct CompletedBuild {
    source: compilation_products::SettledPackageBuild,
}

impl CompletedBuild {
    pub fn package_inputs(&self) -> &package_compilation::PackageCompilationInputs {
        &self.source.inputs
    }
}

impl PackageBuildInvocation {
    /// Join the final Product target and package custody after Build declares
    /// its identity. Completion compares the original acquired capture and
    /// executed role; a context admitted before Build remains fixed.
    pub fn join_product_context(
        &mut self,
        target: target::TargetProfile,
        inputs: package_compilation::PackageCompilationInputs,
    ) -> Result<(), CompileFailure> {
        self.source.join_product_context(target, inputs)
    }

    pub fn execution(&mut self) -> Option<&mut build_evaluation::PreparedBuildExecution> {
        self.source.execution()
    }
}

impl PackageBuild {
    /// Reuse this completed activation for an admitted product target with
    /// equal source and semantic inputs. The completed observations are shared;
    /// permissions remain on each requested product.
    pub fn select_target(
        &self,
        target: target::TargetProfile,
        inputs: package_compilation::PackageCompilationInputs,
    ) -> Result<Self, CompileFailure> {
        self.source
            .select_target(target, inputs)
            .map(|source| Self { source })
            .map_err(CompileFailure::from)
    }

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
    /// Consume the exact activation return and settle all source/output duties.
    /// Authored Product source remains deferred; later admissions are separate
    /// judgments even when default Build already prepared a source prefix.
    pub fn settle_package_build(
        &mut self,
        invocation: PackageBuildInvocation,
        returned: Option<build_evaluation::ReturnedBuild>,
    ) -> Result<CompletedBuild, CompileFailure> {
        super::run_on_compile_thread(|| {
            self.settle_build(invocation.source, returned)
                .map(|source| CompletedBuild { source })
        })
    }

    /// Prepare one Product context from settled Build output, without effects.
    /// Source-routing and semantic admissions are checked in this context rather
    /// than copied from a sibling Product judgment.
    pub fn prepare_package_product(
        &mut self,
        completed: &mut CompletedBuild,
        target: target::TargetProfile,
        inputs: package_compilation::PackageCompilationInputs,
    ) -> Result<PackageBuild, CompileFailure> {
        super::run_on_compile_thread(|| {
            super::checking::prepare_settled_package_product(
                completed.source.fork(),
                target,
                inputs,
            )
            .map(|source| PackageBuild { source })
        })
    }

    /// Admit an occurrence without executing Build or loading Product imports.
    /// An acquired unbound capture needs no provisional package identity. Its
    /// final package custody must join before completion prepares Product.
    pub fn begin_package_build(
        &mut self,
        request: CheckedCompileRequest,
    ) -> Result<PackageBuildInvocation, CompileFailure> {
        super::run_on_compile_thread(|| {
            self.begin_build(request)
                .map(|source| PackageBuildInvocation { source })
        })
    }

    /// Rejoin the exact returned activation, then prepare its Product context.
    /// Outstanding dependency requests still refuse until coordinator settlement
    /// supplies their admitted custody; this does not silently accept a partial join.
    pub fn complete_package_build(
        &mut self,
        invocation: PackageBuildInvocation,
        returned: Option<build_evaluation::ReturnedBuild>,
    ) -> Result<PackageBuild, CompileFailure> {
        super::run_on_compile_thread(|| {
            self.complete_build(invocation.source, returned)
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
