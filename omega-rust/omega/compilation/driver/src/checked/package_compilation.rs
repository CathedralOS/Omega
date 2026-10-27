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
    pub occurrences: Vec<PackageSourceRequest>,
    pub product: crate::CompilationProduct,
    /// Present only for a locked root Product. No lock supplies no permissions;
    /// a missing occurrence inside an existing lock must reject at admission.
    pub terminal_permissions:
        Option<package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
}

/// Execution and Product admission are distinct inputs to one package plan.
/// A Product request borrows an already settled Build; missing custody is an
/// error, never a request to execute the source again.
pub enum PackageSourceRequest {
    Execute(CheckedCompileRequest),
    Product(PackageProductSourceRequest),
}

impl PackageSourceRequest {
    pub fn package_inputs(&self) -> Option<&package_compilation::PackageCompilationInputs> {
        match self {
            Self::Execute(request) => request.package_inputs.as_ref(),
            Self::Product(request) => Some(&request.inputs),
        }
    }

    pub fn set_optimization_rollback(&mut self, rollback: optimization_core::OptimizationRollback) {
        match self {
            Self::Execute(request) => request.optimization_rollback = rollback,
            Self::Product(request) => request.options.optimization_rollback = rollback,
        }
    }
}

/// Product context and the exact pre-effect obligations its retained Build
/// must have satisfied. Product checking options cannot grant Build effects.
#[derive(Clone)]
pub struct PackageProductSourceRequest {
    pub target: target::TargetProfile,
    pub inputs: package_compilation::PackageCompilationInputs,
    pub execution: package_compilation::build_execution::BuildExecutionContract,
    pub options: PackageProductOptions,
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

/// An exact acquired source's observed declaration, not a successful Build or
/// Product. A workspace catalog may inspect names without building unrelated
/// member dependency closures. Selected Builds still owe their full final join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBuildDeclaration {
    capture: package_compilation::CapturedLocalSource,
    declaration: build_declarations::BuildDeclaration,
    settled: bool,
}

impl SourceBuildDeclaration {
    pub(super) fn new(
        capture: package_compilation::CapturedLocalSource,
        declaration: build_declarations::BuildDeclaration,
    ) -> Self {
        Self {
            capture,
            declaration,
            settled: false,
        }
    }

    pub fn declaration(&self) -> &build_declarations::BuildDeclaration {
        &self.declaration
    }

    pub fn source_capture(&self) -> &package_compilation::CapturedLocalSource {
        &self.capture
    }

    pub fn is_settled(&self) -> bool {
        self.settled
    }

    pub(super) fn settled(
        capture: package_compilation::CapturedLocalSource,
        declaration: build_declarations::BuildDeclaration,
    ) -> Self {
        Self {
            capture,
            declaration,
            settled: true,
        }
    }
}

/// Settled Build effects and captured source, before Product preparation.
/// Independent Product admissions borrow this output; none can replay Build or
/// replace its source graph, execution inputs, output bytes or observations.
pub struct CompletedBuild {
    source: compilation_products::SettledPackageBuild,
    products: Vec<PreparedPackageBuild>,
}

/// Product judgment policy, distinct from the already settled Build execution.
/// Discovery may leave service fields unsettled; a bound Product must check
/// them under its own admission and cannot reuse that relaxed judgment.
#[derive(Clone, Default)]
pub struct PackageProductOptions {
    pub optimization_rollback: optimization_core::OptimizationRollback,
    pub permit_unsettled_fused_service_fields: bool,
}

impl CompletedBuild {
    pub fn package_inputs(&self) -> &package_compilation::PackageCompilationInputs {
        &self.source.inputs
    }

    /// Targets whose entry slots this activation actually bound, in execution
    /// order. This observation does not admit the selected implementation or
    /// replace target-specific Product checking.
    pub fn bound_program_targets(&self) -> Result<Vec<target::TargetProfile>, Vec<Diagnostic>> {
        let mut targets = Vec::new();
        for binding in &self
            .source
            .source
            .evaluated
            .configuration
            .root_binding_requests
        {
            let Some((owner, name)) = binding.slot.rsplit_once("::") else {
                return Err(vec![Diagnostic::error(format!(
                    "root slot `{}` is not target-qualified",
                    binding.slot
                ))]);
            };
            let profile =
                target::TargetProfile::from_root_slot_owner(owner).map_err(|error| vec![error])?;
            let required = profile.required_root_slot(name).ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "target profile `{}` declares no required root slot `{}`",
                    profile.target_name(),
                    binding.slot
                ))]
            })?;
            if required.program_entry().is_some() && !targets.contains(&profile) {
                targets.push(profile);
            }
        }
        Ok(targets)
    }

    /// A Product demand cannot retroactively add inputs or required outputs to
    /// an execution. Its contract must be the one supplied before effects.
    pub fn require_execution_contract(
        &self,
        required: &package_compilation::build_execution::BuildExecutionContract,
    ) -> Result<(), CompileFailure> {
        if &self.source.source.execution_contract != required {
            return Err(vec![Diagnostic::error(
                "Product preparation requires different Build execution inputs or output obligations",
            )].into());
        }
        Ok(())
    }
}

impl PackageBuildInvocation {
    pub fn product_queries(&self) -> super::checking::ProductQueries {
        self.source.product_queries()
    }
    pub fn observe_declaration(
        &self,
        returned: &build_evaluation::ReturnedBuild,
    ) -> Result<SourceBuildDeclaration, CompileFailure> {
        self.source.observe_declaration(returned)
    }
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
    /// Finish a workspace catalog, which has no Product owner or dependency
    /// closure of its own. Output duties and exact-return custody still settle.
    pub fn settle_source_catalog(
        &mut self,
        invocation: PackageBuildInvocation,
        returned: build_evaluation::ReturnedBuild,
    ) -> Result<SourceBuildDeclaration, CompileFailure> {
        super::run_on_compile_thread(|| invocation.source.settle_catalog(returned))
    }
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
                .map(|source| CompletedBuild {
                    source,
                    products: Vec::new(),
                })
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
        options: PackageProductOptions,
    ) -> Result<PackageBuild, CompileFailure> {
        super::run_on_compile_thread(|| {
            if let Some(prepared) = completed.products.iter().find(|prepared| {
                prepared.has_source_admission(
                    &inputs,
                    &options.optimization_rollback,
                    options.permit_unsettled_fused_service_fields,
                )
            }) {
                return prepared
                    .select_target(target, inputs)
                    .map(|source| PackageBuild { source })
                    .map_err(CompileFailure::from);
            }
            let mut source = completed.source.fork();
            source.rollback = options.optimization_rollback;
            source.permit_unsettled_fused_service_fields =
                options.permit_unsettled_fused_service_fields;
            let source = super::checking::prepare_settled_package_product(source, target, inputs)?;
            completed.products.push(source.clone());
            Ok(PackageBuild { source })
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

    /// Produce the exact public helper selected by a waiting Build. Checking
    /// retains every implementation; native/default-provider settlement is not
    /// part of this operation. The dependency's own Build must already be joined.
    pub fn produce_package_build_provider(
        &mut self,
        build: PackageBuild,
        declaration_path: &str,
    ) -> Result<psi_compiler::ProducedBuildProvider, CompileFailure> {
        super::run_on_compile_thread(|| {
            let (_, _, inputs, rollback, prepared) = build.source.into_continuation();
            let profile = prepared
                .build_sources
                .build_execution_profile
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "Build provider production requires its admitted execution profile",
                    )]
                })?;
            let source = super::checking::complete_prepared_build(prepared, Some(&inputs))?;
            let checked = super::checking::check_product_source(
                source,
                Some(&inputs),
                &rollback,
                self.reuse(),
                &mut self.selections,
            )?;
            self.retain(&checked.retained_checks);
            // A helper has no installed runtime providers yet. Close its source
            // obligations under the explicit host model without selecting a
            // dependency's Product defaults. Provider-dependent constants still
            // require real selected facts; an empty context cannot prove them.
            let program =
                crate::execution_settlement::constant_recipe_closure::close_constant_recipes(
                    &checked,
                    profile,
                    &checked
                        .computed_build_config
                        .config
                        .opaque_representation_selections,
                    &Default::default(),
                    &[],
                    |_| Ok(Default::default()),
                )?;
            Ok(psi_compiler::BuildProviderProductionRequest {
                checked: &program,
                exact_toolchain_sources: checked.sources.exact_toolchain_sources(),
                package: inputs.root(),
                declaration_path,
            }
            .produce()?)
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
            let mut requests = Vec::new();
            for (owner, selected) in self.select_build_batch(builds) {
                let request = match selected.install().result {
                    Ok(checked) => Ok(crate::product_execution::ProductRequest::Component(checked)),
                    Err(failure) => Err(crate::CompilationProductError::Production(
                        failure.into_diagnostics(),
                    )),
                };
                requests.push((owner, request));
            }
            let mut completed = Vec::new();
            for (owner, product) in crate::compiler::produce_products(requests) {
                let description = match product.and_then(crate::CompilationOutput::into_component) {
                    Ok(product) => {
                        let description = product.description().clone();
                        self.retain_component_product(product);
                        Ok(description)
                    }
                    Err(error) => Err(error.into_diagnostics()),
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
                    .map(
                        |realization| crate::product_execution::ProductRequest::Requested {
                            realization,
                            root,
                            product,
                            permissions,
                        },
                    )
                    .map_err(|failure| {
                        crate::CompilationProductError::Production(failure.into_diagnostics())
                    });
                products.push((owner, request));
            }
            Ok(crate::compiler::produce_products(products))
        })
    }
}
