//! An authored Build returns its declaration and target selections to the project host.

#[path = "../../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;

use build_declarations::BuildDeclaration;
use package_compilation::BuildExecutionInputs;
use package_manager::{
    operations::begin_local_project_preparation, resolution::source::ExecutedSourceResolution,
};
use project_build::{PlanningRequest, PlanningSources, interpret, prepare};
use semantic_vocabulary::NativeBindingLocator;
use std::sync::OnceLock;
use target::TargetProfile;

#[test]
fn alpha_service_defaults_are_returned_by_the_standard_library_build() {
    use semantic_vocabulary::source_selections::ProductTarget;
    use source_declarations::declaration_selection::BuildOperation;

    configure_library();
    let build_path = fixture_repository::root().join("source/library/std/build.omg");
    let standard_package =
        begin_local_project_preparation(&build_path, &[TargetProfile::AlphaBootstrap], true)
            .unwrap()
            .acquire_root()
            .unwrap();
    let files = standard_package.source_capture().tree();
    let standard = files
        .read_to_string(std::path::Path::new("build.omg"))
        .unwrap();
    let exit =
        "Build::select_provider::<ProcessExit, NativeProcessExit>(builder, alpha_bootstrap);";
    let console = "Build::select_provider::<Console, AlphaConsole>(builder, alpha_bootstrap);";
    assert_eq!(standard.matches(exit).count(), 1);
    assert_eq!(standard.matches(console).count(), 1);
    let selected_console = ("Console", "AlphaConsole", "configure_console");
    let selected_exit = ("ProcessExit", "NativeProcessExit", "configure_process_exit");
    for (source, expected) in [
        (standard.as_ref(), vec![selected_console, selected_exit]),
        (&standard.replace(exit, ""), vec![selected_console]),
        (&standard.replace(console, ""), vec![selected_exit]),
    ] {
        let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
            "std-alpha-service-selections",
            false,
        )
        .unwrap();
        for (path, _) in files
            .entries()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "omg"))
        {
            let destination = temp.path().join(path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::write(destination, files.read_to_string(path).unwrap().as_bytes()).unwrap();
        }
        let root = temp.path().join("build.omg");
        std::fs::write(&root, source).unwrap();
        let project =
            begin_local_project_preparation(&root, &[TargetProfile::AlphaBootstrap], true).unwrap();
        let acquired = project.acquire_root().unwrap();
        let prepared = prepare(
            PlanningRequest {
                root: &acquired.entry_path(),
                sources: PlanningSources::Captured(acquired.source_capture()),
                execution: BuildExecutionInputs::default(),
                artifact_only: false,
            },
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let evaluated = interpret(prepared, &mut ())
            .unwrap()
            .complete(None)
            .unwrap();
        let mut alpha = Vec::new();
        for event in &evaluated.configuration.product_selection_requests {
            if event.target == Some(ProductTarget::AlphaBootstrap)
                && event.request.operation == BuildOperation::ProviderSelection
            {
                let [service, provider] = event.request.operands.as_ref() else {
                    panic!("provider selection needs its service and provider");
                };
                if matches!(service.path.as_str(), "Console" | "ProcessExit") {
                    let span = event
                        .selecting_machine
                        .expect("provider selection author")
                        .span;
                    let author = &source[span.start..span.end];
                    alpha.push((service.path.as_str(), provider.path.as_str(), author));
                }
            }
        }
        alpha.sort_unstable();
        assert_eq!(alpha, expected);
    }
}

#[test]
fn authored_build_returns_its_application_and_target_bindings() {
    configure_library();
    let fixture = fixture_repository::root()
        .join("tests/fixtures/providers/run/native_boundary_supply/main.omg");
    let project =
        begin_local_project_preparation(&fixture, &[TargetProfile::LinuxX64], true).unwrap();
    let acquired = project.acquire_root().unwrap();
    let capture = acquired.source_capture();
    let root = acquired.entry_path();
    let evaluation = prepare(
        PlanningRequest {
            root: &root,
            sources: PlanningSources::Captured(capture),
            execution: BuildExecutionInputs::default(),
            artifact_only: false,
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();

    let mut stage = interpret(evaluation, &mut ()).unwrap();
    let Some(BuildDeclaration::Application(application)) = stage.returned().declaration() else {
        panic!("the captured Build must return an application");
    };
    assert_eq!(application.name.as_str(), "native_boundary_supply");
    let observed = stage.observe_source_declaration().unwrap();
    assert_eq!(
        observed.source_capture().normalized().content_identity,
        capture.normalized().content_identity
    );
    assert!(!observed.is_settled());
    // The same source bytes under a different requested inventory do not own this return.
    use package_compilation::{BuildSourceCaptureObligation, BuildSourceCaptureRequest};
    let selected = BuildSourceCaptureRequest::new([
        (b"main.omg".to_vec(), BuildSourceCaptureObligation::Required),
        (
            b"build.omg".to_vec(),
            BuildSourceCaptureObligation::Required,
        ),
        (
            b"unused.omg".to_vec(),
            BuildSourceCaptureObligation::Optional,
        ),
    ])
    .unwrap();
    let scoped = project.acquire_selected_root(&selected).unwrap();
    assert_eq!(scoped.source_capture().tree(), capture.tree());
    assert_ne!(
        scoped.source_capture().selection_identity(),
        capture.selection_identity()
    );
    assert!(scoped.declare_build(stage.returned()).is_err());
    assert!(
        scoped
            .declare_source_result(&stage.source_result().unwrap())
            .is_err()
    );
    let declaration = acquired.declare_build(stage.returned()).unwrap();
    let subject = declaration.identity();
    let ExecutedSourceResolution::Complete(graph) = declaration.resolve().unwrap() else {
        panic!("the fixture declares no dependencies")
    };
    let inputs = graph
        .join_source_result(subject, &mut stage.source_result().unwrap())
        .unwrap();
    let product = stage.open_product(Some(&inputs)).unwrap();
    assert!(product.source().product_sources.is_none());
    assert!(
        product
            .source()
            .parsed
            .files
            .selected_files()
            .any(|file| file.path == root)
    );
    assert_eq!(product.package_inputs().unwrap().root(), subject);
    let planned = product.product_stage(&[TargetProfile::LinuxX64]).unwrap();
    let program = planned
        .program(TargetProfile::LinuxX64, Vec::new())
        .unwrap();
    let compiler::Program::Source { source, target, .. } = &program else {
        panic!("the named project must produce a source request");
    };
    assert_eq!(source.product.subject, Some(subject));
    assert_eq!(source.product.calling_targets, &["linux_x86_64"]);
    assert!(std::ptr::eq(
        source.product.build_accounting.unwrap(),
        &product.evaluated().source_completion_accounting,
    ));
    assert_eq!(
        target.settings.application.name.as_deref(),
        Some("native_boundary_supply")
    );
    assert_eq!(source.product.policy.native_bindings.len(), 4);
    assert!(source.product.native_bindings.evaluation.is_some());
    let computed = &product.evaluated().configuration;
    let [checked_build] = computed.accepted_builds.as_slice() else {
        panic!("this Build executes one published program");
    };
    assert_eq!(
        product.evaluated().subject_validation,
        build_execution::BuildSubjectValidation::Checked
    );
    let checked = checked_build.view();
    assert!(checked.closed_module().is_some());
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = checked.module().entry else {
        panic!("the authored Build selects a checked source machine");
    };
    assert!(
        checked
            .module()
            .machine(entry)
            .unwrap()
            .declaration
            .is_some()
    );
    assert_eq!(
        source
            .product
            .native_bindings
            .evaluation
            .as_ref()
            .unwrap()
            .terminal_code,
        checked.custody_identity().semantic()
    );
    assert_eq!(computed.root_binding_requests.len(), 5);
    assert!(
        computed
            .root_binding_requests
            .iter()
            .any(|binding| binding.slot == "alpha_bootstrap::ProgramEntry")
    );
    assert_eq!(computed.native_binding_requests.len(), 4);
    for (target, syscall) in [("linux_x86_64", 39), ("linux_arm64", 172)] {
        let binding = computed
            .native_binding_requests
            .iter()
            .find(|row| row.target.canonical_name() == target)
            .expect("Build issued the target's native binding");
        assert_eq!(
            binding.locator,
            NativeBindingLocator::Syscall { number: syscall }
        );
    }
    let compiler::Artifact::Native(output) = compiler::compile(
        compiler::CompileRequest::Native {
            program,
            entry_memory: &[],
        },
        Default::default(),
    )
    .expect("the selected getpid binding must reach the native emitter")
    .artifact
    else {
        panic!("the Linux request must yield a native executable");
    };
    let artifact = output.retained_native_artifact().unwrap();
    assert!(artifact.image().output().bytes.starts_with(b"\x7fELF"));
    let module = artifact.psi_artifact().semantic_module();
    let [entry] = module.interfaces.program_entry_contracts.as_slice() else {
        panic!("the selected Linux entry must be the only published physical entry");
    };
    assert_eq!(entry.target, "linux_x86_64");
    let [boundary] = module.interfaces.boundary_machines.as_slice() else {
        panic!("getpid must be the only reached native requirement");
    };
    assert!(
        module
            .calling_abi_identity(boundary.id)
            .unwrap()
            .requirement_identity
            .contains("HostProcessId::get")
    );
    // A saved program enters with the receiving host's installation. The
    // source Build's syscall choice cannot authorize an otherwise empty host.
    let reopened = std::sync::Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.psi_artifact().to_bytes())
            .unwrap(),
    );
    let profile = TargetProfile::LinuxX64;
    let host = psi_authorization::HostProducerTrust::accept("checked project fixture");
    let authorized = psi_authorization::authorize_terminal_artifact(reopened, Some(&host)).unwrap();
    let settings = Default::default();
    let saved = compiler::Program::Psi {
        target: compiler::NativeTarget {
            profile,
            installation: compiler::TargetProviderInstallation::new(
                authorized.identity(),
                profile,
                Default::default(),
            ),
            requested_boundaries: Vec::new(),
            application: Default::default(),
            optimizations: &settings,
            behavior_exclusions: &[],
            x86_scalar_fma_provider: None,
        },
        program: authorized,
    };
    let refusal = compiler::compile(
        compiler::CompileRequest::Native {
            program: saved,
            entry_memory: &[],
        },
        Default::default(),
    )
    .err()
    .expect("a saved program needs the receiving host's process-ID provider")
    .into_diagnostics();
    assert!(
        refusal
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no provider")
                && diagnostic.message.contains("HostProcessId")),
        "{refusal:?}",
    );
}

fn configure_library() {
    static CONFIGURED: OnceLock<()> = OnceLock::new();
    CONFIGURED.get_or_init(|| {
        package_compilation::toolchain_library::configure(
            &fixture_repository::root().join("source/library"),
        )
        .unwrap()
    });
}

#[test]
fn a_return_from_a_checked_arm_reports_the_unavailable_declarations() {
    configure_library();
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-conditional-return",
        false,
    )
    .unwrap();
    let root = temporary.path();
    let entry = root.join("build.omg");
    std::fs::write(root.join("main.omg"), "machine main() {}\n").unwrap();
    std::fs::write(&entry, r#"use kit;
machine build(builder: @mutable Build) suspends {
    Build::package(builder, "conditional");
    builder.tests.disabled = false;
    transition builder.tests.disabled {
        true -> suspend later(builder),
        _ -> done()
    }
    machine later(builder: @mutable Build) suspends {
        let mutable request: DependencyDescription = dependency\from_source(Source::Path { location: "vendor" });
        dependency\with_alias(@mutable request, "kit");
        let registered: DependencyRegistration = Build::build_dependency(builder, request);
        suspend Build::prepare_dependency(builder, registered);
        let value: i32 = kit\v();
    }
    machine done() {}
}"#).unwrap();
    let profile = TargetProfile::host_if_supported().unwrap();
    let project = begin_local_project_preparation(&entry, &[profile], true).unwrap();
    let acquired = project.acquire_root().unwrap();
    let evaluation = prepare(
        PlanningRequest {
            root: &acquired.entry_path(),
            sources: PlanningSources::Captured(acquired.source_capture()),
            execution: BuildExecutionInputs::default(),
            artifact_only: false,
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut stage = interpret(evaluation, &mut ()).unwrap();
    let declaration = acquired.declare_build(stage.returned()).unwrap();
    let subject = declaration.identity();
    let ExecutedSourceResolution::Complete(graph) = declaration.resolve().unwrap() else {
        panic!("the selected arm registered no package")
    };
    let inputs = graph
        .join_source_result(subject, &mut stage.source_result().unwrap())
        .unwrap();
    let product = stage.open_product(Some(&inputs)).unwrap();
    let evaluated = product.evaluated();
    assert_eq!(
        evaluated.subject_validation,
        build_execution::BuildSubjectValidation::CheckedAvailableDeclarations
    );
    let [only] = evaluated.configuration.accepted_builds.as_slice() else {
        panic!("the selected arm returned in the first program")
    };
    assert!(only.view().closed_module().is_none());
    assert!(!only.view().pending_normal_completions().is_empty());
}

#[test]
fn later_build_admissions_keep_the_original_artifact_only_consent() {
    use build_execution::{BuildEvaluationRequest, BuildStageRequest, RestrictedBuildGrants};
    use package_compilation::build_execution::{RestrictedBuildOperation, RestrictedBuildRequest};
    use package_compilation::source_assembly::ImmutableSourceParseCheckpoint;
    use std::sync::{Arc, Mutex};

    struct Consent(Arc<Mutex<Vec<RestrictedBuildRequest>>>);
    impl RestrictedBuildGrants for Consent {
        fn admit(
            &mut self,
            request: &RestrictedBuildRequest,
        ) -> Result<(), Vec<diagnostics::Diagnostic>> {
            self.0.lock().unwrap().push(request.clone());
            Ok(())
        }
    }

    configure_library();
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-artifact-consent",
        false,
    )
    .unwrap();
    let root = temporary.path();
    let entry = root.join("build.omg");
    std::fs::write(root.join("main.omg"), "machine main() {}\n").unwrap();
    std::fs::write(
        &entry,
        r#"machine build(builder: @mutable Build) {
        Build::application(builder, "artifacts");
        Build::artifact_only(builder);
        _ = BuildSource::resolve(@builder.source, "main.omg");
    }"#,
    )
    .unwrap();
    let profile = TargetProfile::host_if_supported().unwrap();
    let project = begin_local_project_preparation(&entry, &[profile], true).unwrap();
    let acquired = project.acquire_root().unwrap();
    let entry = acquired.entry_path();
    let mut timings = Default::default();
    let mut catalogue = Default::default();
    let checkpoint = ImmutableSourceParseCheckpoint::prepare(
        &entry,
        None,
        Some(acquired.source_capture()),
        &mut timings,
        &mut catalogue,
    )
    .unwrap();
    let (_, syntax) = checkpoint
        .assemble(None, &mut timings, &mut catalogue)
        .unwrap();
    let interval = syntax
        .build_stage_source()
        .unwrap()
        .prepare(None, &Default::default(), profile.target_name(), None)
        .unwrap();
    let check_only = syntax
        .check_only_build_stage_source()
        .unwrap()
        .expect("an anonymous root has a check-only Build stage")
        .prepare(None, &Default::default(), profile.target_name(), None)
        .unwrap();
    let staging = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-reviewed-staging",
        false,
    )
    .unwrap();
    let sponsor = filesystem_authority::FilesystemSponsor::new(staging.path()).unwrap();
    let owner = syntax
        .anonymous_owner
        .as_ref()
        .expect("the acquired source owner");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut evaluation = build_execution::prepare_build_evaluation(
        BuildEvaluationRequest {
            source_acquisition: None,
            execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose::Source,
            root_path: &entry,
            interval: &interval,
            check_only_subject: Some(&check_only),
            build_source: syntax.build_source,
            sources: &syntax.parsed.files,
            package_inputs: None,
            anonymous_source: Some(owner),
            generated_source_custody: &[],
            execution: BuildExecutionInputs {
                profile: Some(profile),
                directory: Some(staging.path().join("output")),
                filesystem_sponsor: Some(sponsor),
                restricted_grants: Some(Box::new(Consent(Arc::clone(&requests)))),
                ..Default::default()
            },
            artifact_only: true,
        },
        &mut timings,
    )
    .unwrap();
    // Admit another checked view against the same reviewed staging root. The earlier
    // consent covers its identical access, even after a second handoff.
    for _ in 0..2 {
        evaluation
            .continue_stage(
                BuildStageRequest {
                    source_acquisition: None,
                    interval: &interval,
                    build_source: syntax.build_source,
                    package_inputs: None,
                    anonymous_source: Some(owner),
                },
                &mut timings,
            )
            .unwrap();
    }
    let requests = requests.lock().unwrap();
    let [request] = requests.as_slice() else {
        panic!("one activation asked for different filesystem consent: {requests:?}");
    };
    assert_eq!(
        request.operation(),
        RestrictedBuildOperation::ScopedFilesystemExecution
    );
    assert!(request.bounds().artifact_only());
}

#[test]
fn the_invoker_supplies_a_library_before_the_build_runs() {
    use build_declarations::{BuildDependencySource, DependencyPurpose, ProjectName};
    use package_manager::project::{BuildLibraryRequest, ProjectInput, open_products};

    configure_library();
    let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
        "supplied-build-library",
        false,
    )
    .unwrap();
    let root = temp.path().join("app");
    let helper = root.join("implementation");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&helper).unwrap();
    std::fs::write(
        helper.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "library"); }"#,
    )
    .unwrap();
    std::fs::write(
        helper.join("helper.omg"),
        "public machine visible() -> bool { true }\nmachine secret() -> bool { true }\n",
    )
    .unwrap();
    std::fs::write(root.join("main.omg"), "machine main() {}\n").unwrap();
    let entry = root.join("build.omg");
    let source = r#"use {authored}/helper.omg as kit;
use main.omg as app;

machine build(builder: @mutable Build) {
    let valid: bool = |kit|::visible();
    transition valid { true -> named(builder), false -> wrong(builder), }
    machine named(builder: @mutable Build) {
        Build::application(builder, "consumer");
        Build::bind(builder, linux_x86_64\ProgramEntry, |app|::main);
    }
    machine wrong(builder: @mutable Build) { Build::application(builder, "wrong"); }
}
"#;
    std::fs::write(&entry, source).unwrap();
    let libraries = [BuildLibraryRequest {
        alias: ProjectName::parse("authored").unwrap(),
        source: BuildDependencySource::Path("implementation".into()),
    }];
    let compile = || {
        open_products(
            ProjectInput::WithBuildLibraries {
                file: &entry,
                libraries: &libraries,
            },
            &[TargetProfile::LinuxX64],
            true,
            false,
            &temp.path().join("out"),
            None,
        )
    };
    let products = compile().unwrap();
    let product = products.select(TargetProfile::LinuxX64).unwrap();
    let inputs = product.package_inputs().unwrap();
    assert_eq!(inputs.package_name(inputs.root()), Some("consumer"));
    let helper = inputs
        .dependency_target_for_purpose(inputs.root(), DependencyPurpose::Build, "authored")
        .unwrap();
    assert_eq!(inputs.package_name(helper), Some("library"));
    assert_eq!(
        inputs.dependency_target_for_purpose(inputs.root(), DependencyPurpose::Product, "authored"),
        None
    );
    assert_eq!(
        inputs
            .dependency_build_handoffs()
            .filter(|handoff| handoff.package() == helper
                && handoff.purpose() == DependencyPurpose::Build)
            .count(),
        1
    );
    std::fs::write(&entry, source.replace("::visible()", "::secret()")).unwrap();
    let diagnostics = compile()
        .err()
        .expect("supplying the library does not expose its private machine");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("private machine of another package")
                && diagnostic
                    .resolved_location
                    .as_ref()
                    .is_some_and(|location| location.path == entry && location.line == 5)
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn dependent_continuation_runs_after_its_package_was_prepared() {
    use build_execution::host::requests::BuildDependencyPreparation;
    use build_execution::{CompileFailure, TerminalBuildActivation};
    use package_compilation::source_imports::ProductQueryDependency;
    use package_manager::resolution::source::{
        AcquiredBuildSource, AcquiredSourceSelection, BuildSourceAcquisitions,
    };
    use project_build::PlanningServices;

    struct Helper<'a> {
        root: &'a AcquiredBuildSource,
        acquisitions: BuildSourceAcquisitions<'a>,
        prepared: usize,
    }

    impl PlanningServices for Helper<'_> {
        fn prepare_dependency(
            &mut self,
            activation: &TerminalBuildActivation,
            request: &BuildDependencyPreparation,
        ) -> Result<ProductQueryDependency, CompileFailure> {
            let selected = self
                .acquisitions
                .acquire_issued(
                    self.root,
                    activation.requests(),
                    request.purpose,
                    request.request_ordinal,
                )
                .unwrap();
            let AcquiredSourceSelection::Source(child) = selected else {
                panic!("the helper is a local package")
            };
            let entry = prepare(
                PlanningRequest {
                    root: &child.entry_path(),
                    sources: PlanningSources::Captured(child.source_capture()),
                    execution: BuildExecutionInputs {
                        dependency_activation: true,
                        ..Default::default()
                    },
                    artifact_only: false,
                },
                Default::default(),
                Default::default(),
            )?;
            let mut returned = interpret(entry, &mut ())?;
            let declared = child.declare_build(returned.returned()).unwrap();
            let subject = declared.identity();
            let ExecutedSourceResolution::Complete(graph) = declared.resolve().unwrap() else {
                panic!("the helper declares no dependencies")
            };
            let inputs = graph
                .join_source_result(subject, &mut returned.source_result()?)
                .unwrap();
            let product = returned.open_product(Some(&inputs))?;
            self.prepared += 1;
            Ok(ProductQueryDependency {
                alias: "support".into(),
                child_build_source: product.evaluated().base_source_consumption_commitment,
                inputs,
                generated: None,
            })
        }

        fn declared_source(
            &mut self,
            activation: &TerminalBuildActivation,
        ) -> Result<Option<package_compilation::PackageCompilationInputs>, CompileFailure> {
            Ok(Some(self.root.declared_source_inputs(activation).unwrap()))
        }
    }

    configure_library();
    let path = fixture_repository::root().join(
        "tests/fixtures/build/fail/product_entry_query_foreign_helper_private_entry/main.omg",
    );
    let project = begin_local_project_preparation(&path, &[TargetProfile::LinuxX64], true).unwrap();
    let mut acquisitions = project.source_acquisitions().unwrap();
    let root = acquisitions
        .retain_root(project.acquire_root().unwrap())
        .unwrap();
    let entry = prepare(
        PlanningRequest {
            root: &root.entry_path(),
            sources: PlanningSources::Captured(root.source_capture()),
            execution: BuildExecutionInputs::default(),
            artifact_only: false,
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut services = Helper {
        root: &root,
        acquisitions,
        prepared: 0,
    };
    let refusal = interpret(entry, &mut services)
        .err()
        .expect("the second stage needs a Product query service");
    assert_eq!(services.prepared, 1);
    let diagnostics = refusal.into_diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("without a granted query service")),
        "{diagnostics:?}"
    );
}

#[test]
fn authored_queries_describe_the_deferred_product_before_build_returns() {
    use build_execution::host::bindings::BuildProductQuery;
    use build_execution::{CompileFailure, TerminalBuildActivation};
    use package_compilation::source_imports::ProductQueryDependency;
    use project_build::PlanningServices;
    use source_compiler::publication::TerminalBuildQueryOperation;

    #[derive(Default)]
    struct QueryServices(Vec<TerminalBuildQueryOperation>);

    impl PlanningServices for QueryServices {
        fn query_dependencies(
            &mut self,
            _activation: &TerminalBuildActivation,
            query: &BuildProductQuery,
        ) -> Result<Vec<ProductQueryDependency>, CompileFailure> {
            self.0.push(query.operation);
            Ok(Vec::new())
        }
    }

    configure_library();
    let fixture = fixture_repository::root()
        .join("tests/fixtures/build/pass/product_description_paths_exit/main.omg");
    let project =
        begin_local_project_preparation(&fixture, &[TargetProfile::LinuxX64], true).unwrap();
    let root = project.acquire_root().unwrap();
    let stage = prepare(
        PlanningRequest {
            root: &root.entry_path(),
            sources: PlanningSources::Captured(root.source_capture()),
            execution: Default::default(),
            artifact_only: false,
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut services = QueryServices::default();
    let returned = interpret(stage, &mut services).unwrap();
    assert!(matches!(returned.returned().declaration(), Some(BuildDeclaration::Application(_))));
    // The authored Build asks a third, invalid entry query if either path is wrong.
    assert_eq!(
        services.0,
        [TerminalBuildQueryOperation::Schema, TerminalBuildQueryOperation::Provider]
    );
}
