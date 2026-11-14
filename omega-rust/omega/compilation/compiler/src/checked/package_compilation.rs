// We complete individual package Build activations (`PackageBuildInvocation`,
// `CompletedBuild`, `PackageBuild`) and their requested source products or
// independent component descriptions on `CompilationSession`. Application Build
// owns dependency ordering and generated-source joins, while this facade keeps
// source continuations, cohort checking, component custody, and Terminal/native
// production inside the compiler.

use super::CompilationSession;
use compilation_products::{
    CheckedCompileRequest, CompileFailure, NativeProductRequest, PreparedPackageBuild,
};
use diagnostics::Diagnostic;

pub struct PackageCompilationRequest {
    pub occurrences: Vec<PackageSourceRequest>,
    pub product: crate::CompilationProduct,
    pub terminal_permissions:
        Option<package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
}

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

#[derive(Clone)]
pub struct PackageProductSourceRequest {
    pub target: target::TargetProfile,
    pub inputs: package_compilation::PackageCompilationInputs,
    pub execution: package_compilation::build_execution::BuildExecutionContract,
    pub options: PackageProductOptions,
}

pub struct PackageCompileFailure {
    pub occurrence: usize,
    pub failure: CompileFailure,
}

pub struct PackageBuild {
    source: PreparedPackageBuild,
}

pub struct PackageBuildInvocation {
    source: super::checking::PreparedPackageInvocation,
}

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

pub struct CompletedBuild {
    source: compilation_products::SettledPackageBuild,
    products: Vec<PreparedPackageBuild>,
}

#[derive(Clone, Default)]
pub struct PackageProductOptions {
    pub optimization_rollback: optimization_core::OptimizationRollback,
}

impl CompletedBuild {
    pub fn package_inputs(&self) -> &package_compilation::PackageCompilationInputs {
        &self.source.inputs
    }

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

    pub fn require_execution_contract(
        &self,
        required: &package_compilation::build_execution::BuildExecutionContract,
    ) -> Result<(), CompileFailure> {
        if &self.source.source.execution_contract != required {
            return Err(vec![Diagnostic::error(
                "Product preparation requires different Build execution inputs or output obligations",
            )]
            .into());
        }
        Ok(())
    }
}

impl PackageBuildInvocation {
    pub fn product_queries(&self) -> super::checking::ProductQueries {
        self.source.product_queries()
    }
    pub fn observe_declaration(
        &mut self,
        returned: Option<&mut build_settings::ReturnedBuild>,
    ) -> Result<SourceBuildDeclaration, CompileFailure> {
        self.source.observe_declaration(returned)
    }
    pub fn source_result<'result>(
        &'result mut self,
        returned: Option<&'result mut build_settings::ReturnedBuild>,
    ) -> Result<build_settings::BuildSourceResult<'result>, CompileFailure> {
        self.source.source_result(returned)
    }
    pub fn join_product_context(
        &mut self,
        target: target::TargetProfile,
        inputs: package_compilation::PackageCompilationInputs,
    ) -> Result<(), CompileFailure> {
        self.source.join_product_context(target, inputs)
    }

    pub fn execution(&mut self) -> Option<&mut build_settings::PreparedBuildExecution> {
        self.source.execution()
    }
}

impl PackageBuild {
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

    pub fn build_handoff(&self) -> &package_compilation::PackageBuildHandoff {
        self.source.build_handoff()
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

    pub fn with_independent_component_descriptions(
        self,
        descriptions: Vec<package_compilation::IndependentComponentDescription>,
    ) -> Result<Self, Vec<package_compilation::PackageCompilationInputError>> {
        self.source
            .with_independent_component_descriptions(descriptions)
            .map(|source| Self { source })
    }
}

pub struct PreparedPackageProduct {
    pub build: PackageBuild,
    pub product: crate::CompilationProduct,
    pub permissions: Option<package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
}

impl CompilationSession {
    pub fn settle_source_catalog(
        &mut self,
        invocation: PackageBuildInvocation,
        returned: Option<build_settings::ReturnedBuild>,
    ) -> Result<SourceBuildDeclaration, CompileFailure> {
        super::run_on_compile_thread(|| invocation.source.settle_catalog(returned))
    }

    pub fn settle_package_build(
        &mut self,
        invocation: PackageBuildInvocation,
        returned: Option<build_settings::ReturnedBuild>,
    ) -> Result<CompletedBuild, CompileFailure> {
        super::run_on_compile_thread(|| {
            self.settle_build(invocation.source, returned)
                .map(|source| CompletedBuild {
                    source,
                    products: Vec::new(),
                })
        })
    }

    pub fn prepare_package_product(
        &mut self,
        completed: &mut CompletedBuild,
        target: target::TargetProfile,
        inputs: package_compilation::PackageCompilationInputs,
        options: PackageProductOptions,
    ) -> Result<PackageBuild, CompileFailure> {
        super::run_on_compile_thread(|| {
            if let Some(prepared) = completed.products.iter().find(|prepared| {
                prepared.has_source_admission(&inputs, &options.optimization_rollback)
            }) {
                return prepared
                    .select_target(target, inputs)
                    .map(|source| PackageBuild { source })
                    .map_err(CompileFailure::from);
            }
            let mut source = completed.source.fork();
            source.rollback = options.optimization_rollback;
            let source = super::checking::prepare_settled_package_product(source, target, inputs)?;
            completed.products.push(source.clone());
            Ok(PackageBuild { source })
        })
    }

    pub fn begin_package_build(
        &mut self,
        request: CheckedCompileRequest,
    ) -> Result<PackageBuildInvocation, CompileFailure> {
        super::run_on_compile_thread(|| {
            self.begin_build(request)
                .map(|source| PackageBuildInvocation { source })
        })
    }

    pub fn complete_package_build(
        &mut self,
        invocation: PackageBuildInvocation,
        returned: Option<build_settings::ReturnedBuild>,
    ) -> Result<PackageBuild, CompileFailure> {
        super::run_on_compile_thread(|| {
            self.complete_build(invocation.source, returned)
                .map(|source| PackageBuild { source })
        })
    }

    pub fn check_package_build(
        &mut self,
        build: PackageBuild,
    ) -> Result<super::CheckedCompilation, CompileFailure> {
        super::run_on_compile_thread(|| self.check_package_build_selecting(build, None))
    }

    // One package check, on the calling compile thread. `frontier` is the
    // optional slot `check_keeping_typed_frontier` reads afterwards.
    pub(super) fn check_package_build_selecting(
        &mut self,
        build: PackageBuild,
        frontier: Option<&mut Option<super::checking::typed_frontier::TypedFrontier>>,
    ) -> Result<super::CheckedCompilation, CompileFailure> {
        let ((), selected) = self
            .select_build_batch(vec![((), build.source)], frontier)
            .pop()
            .ok_or_else(|| vec![Diagnostic::error("package checking lost its selected root")])?;
        selected.finish_check().result
    }

    // Provider publication needs the lowered route's actual provider custody.
    // The removed producer cannot be called with typed source.
    pub fn produce_package_build_provider(
        &mut self,
        _build: PackageBuild,
        _declaration_path: &str,
    ) -> Result<psi_compiler::ProducedBuildProvider, CompileFailure> {
        Err(vec![Diagnostic::error(
            "no product route yet: package Build provider publication requires lowered-route provider custody",
        )]
        .into())
    }

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
            let mut completed = Vec::new();
            let mut pending = Vec::new();
            for (position, (owner, build)) in builds.into_iter().enumerate() {
                let cache = super::component_cache::ComponentProductCache::for_build(&build.source)
                    .ok()
                    .flatten();
                if let Some(cache) = &cache
                    && let Ok(Some(product)) = cache.load()
                {
                    let description = product.description().clone();
                    self.retain_component_product(product);
                    completed.push((position, owner, Ok(description)));
                    continue;
                }
                pending.push(((position, owner, cache), build.source));
            }
            let mut requests = Vec::new();
            for (owner, selected) in self.select_build_batch(pending, None) {
                let request = match selected.result {
                    Ok(selected) => selected
                        .into_checked_compilation()
                        .map(crate::product_execution::ProductRequest::Component)
                        .map_err(crate::CompilationProductError::Production),
                    Err(failure) => Err(crate::CompilationProductError::Production(
                        failure.into_diagnostics(),
                    )),
                };
                requests.push((owner, request));
            }
            for ((position, owner, cache), product) in
                crate::target_batch::produce_products(requests)
            {
                let description = match product.and_then(crate::CompilationOutput::into_component) {
                    Ok(product) => {
                        if let Some(cache) = cache {
                            let _ = cache.store(&product);
                        }
                        let description = product.description().clone();
                        self.retain_component_product(product);
                        Ok(description)
                    }
                    Err(error) => Err(error.into_diagnostics()),
                };
                completed.push((position, owner, description));
            }
            completed.sort_unstable_by_key(|(position, _, _)| *position);
            Ok(completed
                .into_iter()
                .map(|(_, owner, result)| (owner, result))
                .collect())
        })
    }

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
                    let inputs = request.build.source.package_inputs().clone();
                    (
                        (owner, root, request.product, request.permissions, inputs),
                        request.build.source,
                    )
                })
                .collect();
            // Each application and each of its separately selected test roots
            // is one product request in the same Terminal batch. Test roots are
            // produced only beside a native application, where completion can
            // execute them before the application report succeeds.
            let mut requests = Vec::new();
            let mut applications = Vec::new();
            for ((owner, root, product, permissions, inputs), selected) in
                self.select_build_batch(builds, None)
            {
                let position = applications.len();
                let test_product = match &product {
                    crate::CompilationProduct::Native(request) => {
                        Some(crate::CompilationProduct::Native(NativeProductRequest {
                            retain_native_inputs: false,
                            image: compilation_products::NativeImageDemand::Emit,
                            private_entry_ram: Vec::new(),
                            ..request.clone()
                        }))
                    }
                    _ => None,
                };
                let mut identities = Vec::new();
                let notes = if test_product.is_some() {
                    selected.test_notes
                } else {
                    Vec::new()
                };
                if let Some(test_product) = test_product {
                    for (identity, result) in selected.tests {
                        requests.push((
                            (position, Some(identities.len())),
                            result
                                .map(|realization| {
                                    crate::product_execution::ProductRequest::Requested {
                                        realization,
                                        package_inputs: Some(inputs.clone()),
                                        root: root.clone(),
                                        product: test_product.clone(),
                                        permissions: permissions.clone(),
                                    }
                                })
                                .map_err(|failure| {
                                    crate::CompilationProductError::Production(
                                        failure.into_diagnostics(),
                                    )
                                }),
                        ));
                        identities.push(identity);
                    }
                }
                let request = selected
                    .result
                    .map(
                        |realization| crate::product_execution::ProductRequest::Requested {
                            realization,
                            package_inputs: Some(inputs.clone()),
                            root,
                            product,
                            permissions,
                        },
                    )
                    .map_err(|failure| {
                        crate::CompilationProductError::Production(failure.into_diagnostics())
                    });
                requests.push(((position, None), request));
                applications.push((owner, identities, notes));
            }
            let mut outputs: Vec<_> = applications
                .iter()
                .map(|(_, identities, _)| (None, Vec::with_capacity(identities.len())))
                .collect();
            for ((position, test), output) in crate::target_batch::produce_products(requests) {
                match test {
                    None => outputs[position].0 = Some(output),
                    Some(_) => outputs[position].1.push(output),
                }
            }
            Ok(applications
                .into_iter()
                .zip(outputs)
                .map(|((owner, identities, notes), (application, tests))| {
                    let application = application.unwrap_or_else(|| {
                        Err(vec![Diagnostic::error(
                            "product batch lost a requested application",
                        )]
                        .into())
                    });
                    let application = application.map(|output| match output {
                        crate::CompilationOutput::Native(mut pending) => {
                            pending.test_notes = notes;
                            pending.tests = identities
                                .into_iter()
                                .zip(tests)
                                .map(|(identity, product)| {
                                    compilation_products::PendingTestProduct {
                                        identity,
                                        product: match product {
                                            Ok(crate::CompilationOutput::Native(pending)) => {
                                                Ok(pending)
                                            }
                                            Ok(_) => Err(vec![Diagnostic::error(
                                                "test root lost its native product",
                                            )]),
                                            Err(error) => Err(error.into_diagnostics()),
                                        },
                                    }
                                })
                                .collect();
                            crate::CompilationOutput::Native(pending)
                        }
                        output => output,
                    });
                    (owner, application)
                })
                .collect())
        })
    }
}
