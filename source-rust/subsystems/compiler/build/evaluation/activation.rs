/*
Retain and step a single source-issued `Build` activation (`TerminalBuildHost`,
`TerminalBuildActivation`, `ReturnedBuild`).

Why `TerminalBuildActivation` is a stateful coroutine owned by the caller rather
than a callback loop:
1. One `TerminalBuildHost` mints a single invocation-local root receiver
   (`original = TerminalStructuralValue::minted(1, ...)`), wires the request,
   binding, and filesystem host adapters (`TerminalBuildRequestHost`,
   `TerminalBuildBindingHost`, `TerminalBuildFilesystemHost`) to that exact
   receiver token, and starts at most one `TerminalBuildActivation` (`started`
   atomic guard). Neither a second instance nor a guest-constructed struct of
   the same type can forge host requests.
2. The project Build driver drives the activation in work slices via `advance(additional_work)`.
   Whenever `advance` pauses on a provider readiness demand (`AwaitingEffect`) or
   a Product declaration query (`HostComputationPending`), the activation stays
   suspended in place with its live `TerminalInstance` and `EvaluationInvocationAccount`
   intact while the driver installs the backing checked before entry or answers the query via
   `install_provider` or `answer_query`.
3. Every mutating transition (`advance`, `install_provider`, `answer_query`) sets
   `self.state = ActivationState::Refused` before performing fallible work and
   restores `Running` or `Returned` only on success, so any partial failure
   permanently poisons the activation rather than letting later steps conceal an
   incomplete provider installation or failed query.
4. On normal return (`complete`), we extract the plain settings from borrowed
   entry `0` (`build_settings::extract`), close the interpreter instance before
   draining host selections and logs, validate the authored project role
   (`project::destination`), and return `ReturnedBuild` with its complete
   request and demanded-source ledgers attached for coordinator source-join
   settlement.
*/

pub(crate) mod preparation;
mod project;
mod provider_installation;
mod source_join;
mod source_result;
pub use source_join::{BuildDependencySelection, DependencyBacking};
pub use source_result::BuildSourceResult;

use build_model::accounting::EvaluationInvocationAccount;
use build_model::work::EvaluationWorkMeter;
use normalized_build_settings::EvaluatedBuildSettings;
use source_compiler::publication::ProducedTerminalProgram;
use source_compiler::representations::declaration_selection::ExecutedBuildSelection;
use source_compiler::representations::declaration_selection::ExecutedEnumExclusion;
use source_compiler::representations::declaration_selection::ExecutedNativeBinding;
use source_compiler::representations::declaration_selection::ExecutedProductSelection;
use source_compiler::representations::declaration_selection::NativeBindingFormation;
use source_compiler::representations::declaration_selection::ProductDeclarationRequest;
use source_compiler::representations::declaration_selection::ProductEntryBinding;
use std::sync::Arc;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstance, TerminalInstantiation,
    TerminalProgram, TerminalProjectedHostAtom, TerminalStructuralInputs, TerminalStructuralValue,
};

use crate::host::bindings::TerminalBuildBindingHost;
use crate::host::requests::{BuildValidationRequest, IssuedBuildRequest, TerminalBuildRequestHost};

pub(crate) fn measurement() -> build_model::contracts::EvaluationMeasurementIdentity {
    build_model::contracts::EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
        storage_schedule: build_model::contracts::TerminalStorageScheduleIdentity::CURRENT,
    }
}

/// Seed `BuildPlan.proof.index_expansion_terms` to the default 64 on the
/// zero-initialized Build receiver so an untouched Build keeps 64 while an
/// authored assignment of 0 is observed and rejected
/// (`wiki/spec/build/configuration.md#proof-work-policy`).
fn default_proof_policy_override(
    program: &TerminalProgram<'_>,
    root_type: semantic_vocabulary::StructuralTypeId,
) -> Result<Option<terminal_interpreter::TerminalStructuralScalarFieldValue>, String> {
    let identities = &program.module().identities;
    let Some(declaration) = program.structural_type(root_type) else {
        return Ok(None);
    };
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Ok(None);
    };
    let Some(proof_field) = fields
        .iter()
        .find(|field| identities.text(field.identity) == Some("proof"))
    else {
        return Ok(None);
    };
    let terminal_psi::StructuralFieldType::Structural(proof_type) = proof_field.field_type else {
        return Err("BuildPlan.proof is not a structural record".to_owned());
    };
    let Some(proof_decl) = program.structural_type(proof_type) else {
        return Err("BuildPlan.proof lost its structural declaration".to_owned());
    };
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &proof_decl.shape else {
        return Err("BuildPlan.proof is not a record".to_owned());
    };
    let Some(terms_field) = fields
        .iter()
        .find(|field| identities.text(field.identity) == Some("index_expansion_terms"))
    else {
        return Err("BuildPlan.proof has no `index_expansion_terms` field".to_owned());
    };
    let scalar_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .map_err(|_| "u64 integer type for BuildPlan.proof.index_expansion_terms".to_owned())?;
    Ok(Some(
        terminal_interpreter::TerminalStructuralScalarFieldValue {
            argument_index: 0,
            path: vec![terminal_psi::StructuralPathSegment::Field(
                proof_field.identity,
            )],
            field: terms_field.id,
            value: terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type,
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                    normalized_build_settings::DEFAULT_PROOF_INDEX_EXPANSION_TERMS,
                )),
            },
        },
    ))
}

fn require_source_acquisition<'a>(
    issued: Option<&'a OriginalBuildSources>,
    expected: &package_compilation::CapturedLocalSource,
) -> Result<&'a package_compilation::PackageBuildEntries, String> {
    issued
        .filter(|source| &source.capture == expected)
        .map(|source| &source.build_entries)
        .ok_or_else(|| {
            "Build declaration does not belong to this original source acquisition".into()
        })
}

/// The receiving source and the libraries selected before its Build was admitted.
pub(crate) struct OriginalBuildSources {
    pub(crate) build_entries: package_compilation::PackageBuildEntries,
    pub(crate) capture: package_compilation::CapturedLocalSource,
    pub(crate) libraries: Arc<[package_compilation::source_imports::SuppliedBuildLibrary]>,
}

pub struct ReturnedBuild {
    pub(crate) preparations: build_model::receipts::DependencyPreparations,
    pub(crate) helper_accounting:
        source_compiler::compile_time_invocations::SourceCompletionAccounting,
    account: EvaluationInvocationAccount,
    original_sources: Option<Arc<OriginalBuildSources>>,
    pub(crate) identity: terminal_codec::TerminalExecutionIdentity,
    destination: Option<project::Destination>,
    pub successor_directories: package_compilation::build_execution::BuildDirectoryMappings,
    successor_directory: Option<package_compilation::build_execution::SuccessorDirectory>,
    pub(crate) source_join: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
    pub(crate) requests: Vec<IssuedBuildRequest>,
    pub(crate) validation_requests: Vec<BuildValidationRequest>,
    pub(crate) validation_subjects: Vec<crate::DependencyValidationSubject>,
    pub(crate) demanded_sources: Vec<BuildDependencySelection>,
    pub(crate) settings: EvaluatedBuildSettings,
    pub(crate) optimization_report: optimization_requests::OptimizationReportRequest,
    pub(crate) enabled_unsafe_atoms:
        std::collections::BTreeSet<language_core::core_vocabulary::UnsafeAtom>,
    pub(crate) root_bindings: Vec<ProductEntryBinding>,
    pub(crate) product_selections: Vec<ExecutedProductSelection>,
    pub(crate) native_formations: Vec<NativeBindingFormation>,
    pub(crate) native_bindings: Vec<ExecutedNativeBinding>,
    pub(crate) exclusions: Vec<ExecutedEnumExclusion>,
    pub(crate) declared_product_selections: Vec<Arc<ProductDeclarationRequest>>,
    pub(crate) output: crate::host::requests::BuildRequestOutput,
    pub(crate) filesystem: Option<package_compilation::build_execution::BuildFilesystemEffects>,
}

impl ReturnedBuild {
    /// Replace this activation's issued coordinates with their frozen subjects after return.
    pub fn resolve_validation_requests(
        &mut self,
        mut resolve: impl FnMut(
            &[IssuedBuildRequest],
            &BuildValidationRequest,
        )
            -> Result<crate::DependencyValidationSubject, crate::CompileFailure>,
    ) -> Result<(), crate::CompileFailure> {
        let prepares_build = self.successor_entry().is_some();
        for request in &self.validation_requests {
            let mut subject = resolve(&self.requests, request)?;
            if subject.purpose != request.purpose() {
                return Err(vec![diagnostics::Diagnostic::error(
                    "validation subject changed its registration purpose",
                )]
                .into());
            }
            // A preparation's immediate consumer is the interpreter for the successor.
            if prepares_build {
                subject.purpose = build_declarations::DependencyPurpose::Build;
            }
            if !self.validation_subjects.contains(&subject) {
                self.validation_subjects.push(subject);
            }
        }
        self.validation_requests.clear();
        Ok(())
    }

    pub fn has_dependency_validation(&self) -> bool {
        !self.validation_subjects.is_empty()
    }

    pub(crate) fn original_acquisition(&self) -> Option<&package_compilation::CapturedLocalSource> {
        self.original_sources
            .as_ref()
            .map(|sources| &sources.capture)
    }

    /// Check that the receiving source acquisition owns this declaration and request ledger.
    pub fn require_source_acquisition(
        &self,
        source: &package_compilation::CapturedLocalSource,
    ) -> Result<&package_compilation::PackageBuildEntries, String> {
        require_source_acquisition(self.original_sources.as_deref(), source)
    }

    pub fn source_result(&mut self) -> BuildSourceResult<'_> {
        BuildSourceResult::executed(self)
    }

    pub fn declaration(&self) -> Option<&build_declarations::BuildDeclaration> {
        self.destination
            .as_ref()
            .and_then(project::Destination::project)
    }

    pub fn successor_entry(&self) -> Option<&build_declarations::BuildEntryPath> {
        match self.destination.as_ref()? {
            project::Destination::Successor(path) => Some(path),
            project::Destination::Project(_) => None,
        }
    }

    pub fn successor_directory(&self) -> Option<&package_compilation::build_execution::SuccessorDirectory> {
        self.successor_directory.as_ref()
    }

    /// The default next source view is the immutable package this Build received.
    /// Issue the selection and its backing from the same completed activation.
    pub fn successor_source(
        &self,
    ) -> Result<
        (
            &build_declarations::BuildEntryPath,
            &package_compilation::CapturedLocalSource,
        ),
        String,
    > {
        let entry = self
            .successor_entry()
            .ok_or("only a preparing Build can supply a successor")?;
        let source = self
            .original_acquisition()
            .ok_or("the preparing Build has no issued source view for its successor")?;
        Ok((entry, source))
    }

    pub fn requests(&self) -> &[IssuedBuildRequest] {
        &self.requests
    }

    pub fn demanded_sources(&self) -> &[BuildDependencySelection] {
        &self.demanded_sources
    }
}

// Empty Output values are ordinary assignable data. Only the host-supplied
// payload, retained separately from their stable storage home, authorizes use.
struct OriginalOutput {
    path: Vec<terminal_psi::StructuralPathSegment>,
    value: TerminalStructuralValue,
}

impl OriginalOutput {
    fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        original: &TerminalStructuralValue,
    ) -> Result<Self, String> {
        let home = crate::host::requests::build_facet(artifact, original, "output")?;
        let declaration = artifact
            .module()
            .types
            .structural_types
            .iter()
            .find(|declaration| declaration.id == home.structural_type)
            .ok_or("Build output facet lost its declaration")?;
        if !matches!(&declaration.shape, terminal_psi::StructuralTypeShape::Record { fields, .. } if fields.is_empty())
        {
            return Err("Build output facet requires an empty record declaration".into());
        }
        let relative = home
            .path
            .strip_prefix(original.path.as_slice())
            .ok_or("Build output facet lost its original receiver path")?;
        let (qualifications, projected_qualifications) =
            terminal_psi::StructuralPathQualification::select(
                &original.qualifications,
                &original.projected_qualifications,
                relative,
            );
        // This is the expected token for this stage's checked declaration.
        // Only the initial invocation places this token in guest storage.
        let mut value = TerminalStructuralValue::minted(
            crate::host::ORIGINAL_OUTPUT_IDENTITY,
            home.structural_type,
            qualifications,
        );
        value.projected_qualifications = projected_qualifications;
        Ok(Self {
            path: relative.to_vec(),
            value,
        })
    }

    fn binding(&self) -> TerminalProjectedHostAtom {
        TerminalProjectedHostAtom {
            argument_index: 0,
            path: self.path.clone(),
            value: self.value.clone(),
        }
    }
}

pub struct TerminalBuildHost {
    original_sources: Option<Arc<OriginalBuildSources>>,
    execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
    produced: Arc<ProducedTerminalProgram>,
    accepted: Arc<source_compiler::publication::AcceptedTerminalSource>,
    providers: Arc<crate::admission::provider_execution::BuildProviderInstallation>,
    account: EvaluationInvocationAccount,
    original: TerminalStructuralValue,
    output: OriginalOutput,
    requests: Arc<TerminalBuildRequestHost>,
    bindings: Arc<TerminalBuildBindingHost>,
    filesystem: Arc<crate::host::filesystem::TerminalBuildFilesystemHost>,
    started: std::sync::atomic::AtomicBool,
    execution_profile: Option<target::TargetProfile>,
    default_source: bool,
    default_sources: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
    machine: Arc<package_compilation::build_execution::SelectedBuildMachine>,
}

impl TerminalBuildHost {
    pub(crate) fn new(
        execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose,
        produced: Arc<ProducedTerminalProgram>,
        accepted: Arc<source_compiler::publication::AcceptedTerminalSource>,
        providers: Arc<crate::admission::provider_execution::BuildProviderInstallation>,
        account: &EvaluationInvocationAccount,
        machine: Arc<package_compilation::build_execution::SelectedBuildMachine>,
        filesystem_scope: &package_compilation::BuildMachineFilesystemScope,
        programs: Arc<crate::host::programs::ProgramInstallation>,
        execution_profile: Option<target::TargetProfile>,
        default_source: bool,
        default_sources: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
        original_sources: Option<Arc<OriginalBuildSources>>,
    ) -> Result<Arc<Self>, String> {
        let artifact = produced.view();
        let parameter = machine.receiver_for(artifact.module())?;
        let program = TerminalProgram::borrow(artifact);
        let entry = program.entry().map_err(describe)?;
        // The evaluator is the build entry's caller, so it owes the entry's
        // `requires` clauses before it runs anything. It hands the entry a
        // Build it promises nothing about, so the only clauses it can owe
        // are ones true of the evaluator itself, such as its address width
        // (`requires address::Bound == 18446744073709551616`).
        match entry.unconditional_precondition()? {
            Some(true) => {}
            Some(false) => {
                return Err(
                    "the build entry's `requires` does not hold for this build evaluator".into(),
                );
            }
            None => {
                return Err(
                    "not supported yet: a build entry `requires` clause over its arguments; \
                     the evaluator establishes nothing about the Build it passes"
                        .into(),
                );
            }
        }
        // The evaluator is the only issuer of the Build it lends (guest code
        // cannot construct one), so the value it presents carries exactly the
        // memberships the entry's formal declares, its sealed fields' included.
        let mut original = TerminalStructuralValue::minted(
            1,
            parameter.structural_type,
            parameter.qualifications.clone(),
        );
        original.projected_qualifications = parameter.projected_qualifications.clone();
        let output = OriginalOutput::new(artifact, &original)?;
        let filesystem = crate::host::filesystem::TerminalBuildFilesystemHost::new(
            &produced,
            &original,
            &output.value,
            &providers.imports,
            account,
            filesystem_scope,
            &machine.name,
            programs.uses_files(),
        )?;
        let requests = TerminalBuildRequestHost::new(
            artifact,
            produced.build_requests(),
            programs,
            Arc::clone(&filesystem),
            original.clone(),
            providers.diagnostic_output_bytes,
            Some(account),
            original_sources
                .as_ref()
                .map(|sources| Arc::clone(&sources.libraries))
                .unwrap_or_default(),
        )?;
        let bindings = TerminalBuildBindingHost::new(
            artifact,
            produced.build_bindings(),
            produced.build_requests(),
            original.clone(),
            Some(account),
            requests.reflected_services(),
        )?;
        Ok(Arc::new(Self {
            original_sources,
            execution_purpose,
            produced,
            accepted,
            providers,
            account: account.clone(),
            original,
            output,
            requests,
            bindings,
            filesystem,
            started: std::sync::atomic::AtomicBool::new(false),
            execution_profile,
            default_source,
            default_sources,
            machine,
        }))
    }

    pub fn start(
        self: &Arc<Self>,
        provide: impl FnMut(
            semantic_vocabulary::PackageKeyIdentity,
            &str,
        ) -> Result<
            source_compiler::publication::ProducedBuildProvider,
            crate::CompileFailure,
        >,
    ) -> Result<TerminalBuildActivation, crate::CompileFailure> {
        let refuse = |reason: String| {
            let mut error = diagnostics::Diagnostic::error(reason);
            error.source_span = self
                .accepted
                .source()
                .symbols
                .symbol_source_span(self.machine.symbol);
            vec![error]
        };
        if self.started.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return Err(refuse("Build host has already started its activation".into()).into());
        }
        let ready_providers =
            provider_installation::check_backings(&self.accepted, &self.produced, provide)?;
        let instance = self.instantiate().map_err(refuse)?;
        let work = build_model::work::sponsored_meter(self.account.clone(), Some(0))
            .map_err(|error| refuse(describe(error)))?;
        artifacts::compile_timings::counters::record_build_execution(self.execution_purpose);
        Ok(TerminalBuildActivation {
            preparations: Default::default(),
            instance,
            host: Arc::clone(self),
            ready_providers,
            work,
            state: ActivationState::Running,
            described_product: None,
            requests: Vec::new(),
            demanded_sources: Vec::new(),
            helper_accounting: Default::default(),
        })
    }

    fn instantiate(self: &Arc<Self>) -> Result<TerminalInstance<'static, 'static>, String> {
        let artifact = self.produced.view();
        let program = TerminalProgram::retain(self.produced.retain_code());
        let request_calls = self
            .produced
            .build_requests()
            .calls_for(artifact)
            .ok_or("Build requests lost their artifact custody")?;
        let binding_calls = self
            .produced
            .build_bindings()
            .calls_for(artifact)
            .ok_or("Build bindings lost their artifact custody")?;
        let query_calls = self
            .produced
            .build_bindings()
            .queries_for(artifact)
            .ok_or("Build queries lost their artifact custody")?;
        let filesystem_calls = self
            .produced
            .build_filesystem()
            .calls_for(artifact)
            .ok_or("Build filesystem calls lost their artifact custody")?;
        let output = self.output.binding();
        let proof_override =
            default_proof_policy_override(&program, self.original.structural_type)?;
        let mut instance = program
            .instantiate(TerminalInstantiation {
                float_evaluation:
                    terminal_interpreter::float_result_policy::FloatEvaluation::CompileTime,
                structural_inputs: TerminalStructuralInputs {
                    arguments: std::slice::from_ref(&self.original),
                    projected_host_atoms: std::slice::from_ref(&output),
                    zero_initialized_arguments: &[0],
                    zero_initialized_scalar_overrides: proof_override.as_slice(),
                    ..Default::default()
                },
                installation: Some(&self.providers.supplies),
                account: Some(&self.account),
                ..Default::default()
            })
            .map_err(describe)?;
        for boundary in instance.unbound_imports().collect::<Vec<_>>() {
            if self.providers.awaited.binary_search(&boundary).is_ok() {
                continue;
            }
            let declaration = artifact
                .module()
                .interfaces
                .boundary_machines
                .iter()
                .find(|declaration| declaration.id == boundary)
                .ok_or("Build import has no Terminal declaration")?;
            if let Some(call) = request_calls
                .iter()
                .find(|call| call.site.boundary == boundary)
            {
                if call.action.configures_test_group() {
                    instance.bind_import(boundary, self.bindings.adapter(declaration.into()))
                } else {
                    instance.bind_import(boundary, self.requests.adapter(declaration.into()))
                }
                .map_err(describe)?;
            } else if binding_calls
                .iter()
                .any(|call| call.site.boundary == boundary)
                || query_calls
                    .iter()
                    .any(|call| call.site.boundary == boundary)
            {
                instance
                    .bind_import(boundary, self.bindings.adapter(declaration.into()))
                    .map_err(describe)?;
            } else if filesystem_calls
                .iter()
                .any(|call| call.site.boundary == boundary)
            {
                instance
                    .bind_import(boundary, self.filesystem.adapter(declaration.into()))
                    .map_err(describe)?;
            } else {
                let operation = artifact
                    .module()
                    .identities
                    .text(declaration.identity)
                    .ok_or("Build import has no callable identity")?;
                return Err(format!(
                    "Build import `{operation}` has no admitted host operation or installed provider"
                ));
            }
        }

        Ok(instance)
    }
}

enum ActivationState {
    Running,
    Returned,
    Refused,
}

pub struct TerminalBuildActivation {
    preparations: build_model::receipts::DependencyPreparations,
    instance: TerminalInstance<'static, 'static>,
    host: Arc<TerminalBuildHost>,
    ready_providers: Vec<(
        crate::host::bindings::BuildProviderRequest,
        source_compiler::publication::ProducedBuildProvider,
    )>,
    work: EvaluationWorkMeter,
    state: ActivationState,
    described_product: Option<(
        build_declarations::BuildDeclarationKind,
        build_declarations::ProjectName,
    )>,
    requests: Vec<IssuedBuildRequest>,
    demanded_sources: Vec<BuildDependencySelection>,
    helper_accounting: source_compiler::compile_time_invocations::SourceCompletionAccounting,
}

/// The single reason the interpreter returned control to the planner.
pub enum TerminalBuildStep {
    Returned,
    NeedsWork,
    Provide,
    Prepare(crate::host::requests::BuildDependencyPreparation),
    Query(Arc<crate::host::bindings::BuildProductQuery>),
}

impl TerminalBuildActivation {
    /// Check the source capture that owns this activation.
    pub fn require_source_acquisition(
        &self,
        source: &package_compilation::CapturedLocalSource,
    ) -> Result<&package_compilation::PackageBuildEntries, String> {
        require_source_acquisition(self.host.original_sources.as_deref(), source)
    }

    pub fn helper_evaluation_accounting(
        &self,
    ) -> &source_compiler::compile_time_invocations::SourceCompletionAccounting {
        &self.helper_accounting
    }

    pub fn answer_query(
        &mut self,
        request: &crate::host::bindings::BuildProductQuery,
        answer: crate::host::bindings::BuildProductDescription,
        dependencies: &[BuildDependencySelection],
    ) -> Result<(), String> {
        if !matches!(self.state, ActivationState::Running) {
            return Err("Product query answer requires a live activation".into());
        }
        self.state = ActivationState::Refused;
        for selected in dependencies {
            if selected.purpose != build_declarations::DependencyPurpose::Product
                || self
                    .requests
                    .iter()
                    .filter(|issued| {
                        matches!(
                            issued.request(),
                            build_declarations::BuildRequest::Dependency {
                                purpose: build_declarations::DependencyPurpose::Product,
                                ..
                            }
                        )
                    })
                    .nth(selected.request_ordinal)
                    .is_none()
            {
                return Err("Product query dependency has no issued Product request".into());
            }
        }
        if request.operation.requires_named_source() {
            let current = self.project_declaration()?;
            let key = project::named_identity(&current)
                .ok_or("Product query requires a named product")?;
            if self
                .described_product
                .as_ref()
                .is_some_and(|issued| issued != &key)
            {
                return Err(
                    "Build changed the product that issued an earlier declaration description"
                        .into(),
                );
            }
            self.described_product = Some(key);
        }
        self.host.bindings.answer_query(request, answer)?;
        self.demanded_sources.extend_from_slice(dependencies);
        self.state = ActivationState::Running;
        Ok(())
    }

    /// A paused declaration query reads the product currently stored in this activation's plan.
    pub fn project_declaration(&self) -> Result<build_declarations::BuildDeclaration, String> {
        let current = project::destination(
            &self.instance,
            &self.host.machine,
            terminal_interpreter::TerminalObservationRoot::ArgumentAtHostCall(0),
            &self.requests,
            &self.host.requests,
        )
        .map_err(|reason| {
            format!("Product declaration query requires a named package or application: {reason}")
        })?;
        current
            .as_ref()
            .and_then(project::Destination::project)
            .cloned()
            .ok_or_else(|| "Build query requires a selected named Product".into())
    }

    pub fn requests(&self) -> &[IssuedBuildRequest] {
        &self.requests
    }

    pub fn view(&self) -> terminal_codec::TerminalExecutionView<'_> {
        self.host.produced.view()
    }

    pub fn install_provider(&mut self) -> Result<(), String> {
        if !matches!(self.state, ActivationState::Running) {
            return Err("Build provider installation requires a live readiness pause".into());
        }
        self.state = ActivationState::Refused;
        let request = self
            .host
            .bindings
            .awaiting_provider()?
            .ok_or("Build has no provider readiness pause")?;
        let provider = self
            .ready_providers
            .iter()
            .find(|(selected, _)| selected == &request)
            .map(|(_, provider)| provider)
            .ok_or("Build provider wait has no checked implementation")?;
        if !self.host.bindings.has_installed_backing(provider)? {
            let profile = self
                .host
                .execution_profile
                .ok_or("Build provider installation has no admitted execution profile")?;
            let bindings = provider_installation::prepare(
                self.host.produced.view(),
                &request,
                provider,
                profile,
                &self.host.account,
                &mut self.helper_accounting,
            )?;
            for binding in bindings {
                self.instance.bind_component(binding)?;
            }
        }
        self.host.bindings.provider_installed(provider)?;
        self.state = ActivationState::Running;
        Ok(())
    }

    /// Answers the pause of `Build::prepare_dependency`: the coordinator
    /// acquired the awaited registration, ran its Build and prepared it as
    /// `selected`. The parked call completes on the next `advance`.
    pub fn dependency_prepared(
        &mut self,
        selected: BuildDependencySelection,
        inputs: [u8; 32],
        child_build_source: Option<[u8; 32]>,
        handoff: Option<[u8; 32]>,
    ) -> Result<(), String> {
        if !matches!(self.state, ActivationState::Running) {
            return Err("Build dependency preparation requires a live readiness pause".into());
        }
        self.state = ActivationState::Refused;
        let awaited = self
            .host
            .requests
            .awaiting_preparation()?
            .ok_or("Build has no dependency preparation pause")?;
        if selected.purpose != awaited.purpose
            || selected.request_ordinal != awaited.request_ordinal
        {
            return Err(
                "prepared dependency differs from the registration the Build waits for".into(),
            );
        }
        self.host.requests.dependency_prepared()?;
        self.preparations.prepared(
            match selected.purpose {
                build_declarations::DependencyPurpose::Product => {
                    source_provenance::DependencyScope::Product
                }
                build_declarations::DependencyPurpose::Build => {
                    source_provenance::DependencyScope::Build
                }
            },
            selected.request_ordinal,
            selected.package,
            inputs,
            child_build_source,
            handoff,
        );
        if !self.demanded_sources.contains(&selected) {
            self.demanded_sources.push(selected);
        }
        self.state = ActivationState::Running;
        Ok(())
    }

    pub fn advance(&mut self, additional_work: u64) -> Result<TerminalBuildStep, String> {
        self.advance_diagnostic(additional_work)
            .map_err(|error| error.message)
    }

    /// Preserve a host refusal's authored call in the executing Build.
    pub fn advance_diagnostic(
        &mut self,
        additional_work: u64,
    ) -> Result<TerminalBuildStep, Box<diagnostics::Diagnostic>> {
        fn refusal(reason: impl Into<String>) -> Box<diagnostics::Diagnostic> {
            Box::new(diagnostics::Diagnostic::error(reason))
        }

        match self.state {
            ActivationState::Returned => {
                return Err(refusal("Build activation has already returned"));
            }
            ActivationState::Refused => return Err(refusal("Build activation has already failed")),
            ActivationState::Running => {}
        }
        self.state = ActivationState::Refused;
        self.work
            .replenish(additional_work)
            .map_err(|error| refusal(describe(error)))?;
        let status = self
            .instance
            .invoke(&mut self.work)
            .map_err(|error| Box::new(self.guest_diagnostic(error)))?;
        self.requests
            .extend(self.host.requests.drain_requests().map_err(refusal)?);
        let step = match status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                TerminalBuildStep::Returned
            }
            TerminalExecutionStatus::SponsorExhausted(_) => TerminalBuildStep::NeedsWork,
            TerminalExecutionStatus::AwaitingEffect(_) => match (
                self.host.bindings.awaiting_provider().map_err(refusal)?,
                self.host.requests.awaiting_preparation().map_err(refusal)?,
            ) {
                (Some(_), None) => TerminalBuildStep::Provide,
                (None, Some(dependency)) => TerminalBuildStep::Prepare(dependency),
                (None, None) => {
                    return Err(refusal(
                        "Build paused without an admitted provider or dependency demand",
                    ));
                }
                (Some(_), Some(_)) => {
                    return Err(refusal(
                        "one Build effect requested both a provider and a dependency",
                    ));
                }
            },
            TerminalExecutionStatus::HostComputationPending(_) => {
                let query = self
                    .host
                    .bindings
                    .awaiting_query()
                    .map_err(refusal)?
                    .ok_or_else(|| refusal("Build paused without an admitted Product query"))?;
                TerminalBuildStep::Query(query)
            }
            other => return Err(refusal(format!("Build did not return normally: {other:?}"))),
        };
        self.state = match &step {
            TerminalBuildStep::Returned => ActivationState::Returned,
            _ => ActivationState::Running,
        };
        Ok(step)
    }

    fn guest_diagnostic(
        &self,
        error: terminal_interpreter::TerminalInstanceError,
    ) -> diagnostics::Diagnostic {
        use terminal_interpreter::{TerminalInstanceError, TerminalInterpretError};
        if let TerminalInstanceError::Execution(TerminalInterpretError::ExecutionSite {
            machine,
            cause,
            ..
        }) = &error
            && let TerminalInterpretError::EffectRejected {
                operation,
                rejection,
            } = cause.as_ref()
        {
            let mut diagnostic = diagnostics::Diagnostic::error(&rejection.reason);
            let accepted = &self.host.accepted;
            if let Some(files) = accepted.source().symbols.source_map() {
                // Build services run in toolchain helpers. Walk the actual callers
                // so repeated uses of one helper still point at the call that ran.
                use terminal_psi::CallSite;
                diagnostic.source_span =
                    std::iter::once((*machine, CallSite::Operation(*operation)))
                        .chain(self.instance.caller_sites())
                        .find_map(|(machine, site)| {
                            let sources = accepted.closure().elaboration().sources.get(machine)?;
                            let span = match site {
                                CallSite::Operation(id) => sources.operations.get(id),
                                CallSite::Edge(id) => sources.edges.get(id),
                            }?
                            .span()?;
                            let file = files.file_at(span)?;
                            (!file.origin.is_toolchain()).then_some(span)
                        });
                package_compilation::attribute_diagnostics(
                    std::slice::from_mut(&mut diagnostic),
                    &files,
                    None,
                );
            }
            return diagnostic;
        }
        diagnostics::Diagnostic::error(describe(error))
    }

    pub fn complete(mut self) -> Result<ReturnedBuild, String> {
        if !matches!(self.state, ActivationState::Returned) {
            return Err("Build completion requires its normal interpreter return".into());
        }
        let (settings, optimization_report) =
            build_settings::extract(&self.instance, &self.host.machine)?;
        let destination = project::destination(
            &self.instance,
            &self.host.machine,
            terminal_interpreter::TerminalObservationRoot::ReturnedArgument(0),
            &self.requests,
            &self.host.requests,
        )?;
        let successor_directory = project::successor_directory(
            &self.instance,
            &self.host.machine,
            &self.host.filesystem,
        )?;
        let successor_directories = project::successor_directories(
            &self.instance,
            &self.host.machine,
            &self.host.filesystem,
        )?;
        if !successor_directories.is_empty()
            && !matches!(destination, Some(project::Destination::Successor(_)))
        {
            return Err("runtime directory handoff requires an interpreted Build product".into());
        }
        if successor_directory.is_some()
            && !matches!(destination, Some(project::Destination::Successor(_)))
        {
            return Err("package_root requires an interpreted Build product".into());
        }
        if self.host.default_source {
            if destination.is_some() || !self.requests.is_empty() {
                return Err("default Build changed the product supplied by its invoker".into());
            }
        } else if destination.is_none() {
            return Err("Build returned no project role".into());
        }
        if let Some(issued) = &self.described_product {
            let returned = destination
                .as_ref()
                .and_then(project::Destination::project)
                .and_then(project::named_identity);
            if returned.as_ref() != Some(issued) {
                return Err("returned Build product differs from the product that issued a declaration description".into());
            }
        }
        self.instance.close().map_err(describe)?;
        drop(self.instance);
        let mut root_bindings = Vec::new();
        let mut product_selections = Vec::new();
        let mut enabled_unsafe_atoms = std::collections::BTreeSet::new();
        let mut exclusions = Vec::new();
        let mut native_bindings = Vec::new();
        for selection in self.host.bindings.take_executed()? {
            match selection {
                ExecutedBuildSelection::EnableUnsafe(atom) => {
                    enabled_unsafe_atoms.insert(atom);
                }
                ExecutedBuildSelection::Entry(binding) => root_bindings.push(binding),
                ExecutedBuildSelection::Exclusion(selection) => exclusions.push(selection),
                ExecutedBuildSelection::Native(selection) => native_bindings.push(selection),
                ExecutedBuildSelection::Declaration(selection) => {
                    product_selections.push(selection)
                }
            }
        }
        let declared_product_selections = self.host.bindings.declared();
        Ok(ReturnedBuild {
            preparations: self.preparations,
            helper_accounting: self.helper_accounting,
            account: self.host.account.clone(),
            original_sources: self.host.original_sources.clone(),
            identity: self.host.produced.view().custody_identity(),
            destination,
            successor_directory,
            successor_directories,
            source_join: self.host.default_sources.clone(),
            requests: self.requests,
            validation_requests: self.host.requests.validation_requests()?,
            validation_subjects: Vec::new(),
            demanded_sources: self.demanded_sources,
            settings,
            optimization_report,
            root_bindings,
            product_selections,
            native_bindings,
            native_formations: self.host.bindings.native_formations(),
            exclusions,
            enabled_unsafe_atoms,
            declared_product_selections,
            output: self.host.requests.take_output()?,
            filesystem: self.host.filesystem.take_effects()?,
        })
    }
}

fn describe(error: impl std::fmt::Debug) -> String {
    format!("Terminal Build: {error:?}")
}
