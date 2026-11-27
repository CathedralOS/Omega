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
    let build_path = fixture_repository::root().join("source/library/system/build.omg");
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
        "Build::select_provider::<|process|::ProcessExit, |process_exit|::NativeProcessExit>(builder, alpha_bootstrap);";
    let console = "Build::select_provider::<|console|::Console, |console|::AlphaConsole>(builder, alpha_bootstrap);";
    assert_eq!(standard.matches(exit).count(), 1);
    assert_eq!(standard.matches(console).count(), 1);
    let selected_console = (
        "|console|::Console",
        "|console|::AlphaConsole",
        "configure_console",
    );
    let selected_exit = (
        "|process|::ProcessExit",
        "|process_exit|::NativeProcessExit",
        "configure_process_exit",
    );
    for (source, expected) in [
        (standard.as_ref(), vec![selected_console, selected_exit]),
        (&standard.replace(exit, ""), vec![selected_console]),
        (&standard.replace(console, ""), vec![selected_exit]),
    ] {
        let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
            "system-alpha-service-selections",
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
                // Rows keep the authored, file-qualified service and provider names.
                if matches!(
                    service.path.as_str(),
                    "|console|::Console" | "|process|::ProcessExit"
                ) {
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
fn successor_runs_the_entry_and_helper_in_the_view_supplied_by_its_invoker() {
    use platform_custody::temporary_directory::TemporaryDirectory;

    configure_library();
    let temporary = TemporaryDirectory::create("issued-successor-view", false).unwrap();
    let root = temporary.path();
    let entry = root.join("build.omg");
    std::fs::create_dir(root.join("generated")).unwrap();
    std::fs::write(
        &entry,
        r#"machine build(builder: @mutable BuildPlan) {
        builder.product = BuildProduct::InterpretedBuild { entry: "generated/configure.omg" };
    }"#,
    )
    .unwrap();
    // An ordinary generator supplies files to this invocation before capture.
    std::fs::write(
        root.join("generated/configure.omg"),
        r#"use generated/policy.omg;
        machine build(builder: @mutable BuildPlan) { |policy|::configure(builder); }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("generated/policy.omg"),
        r#"public machine configure(builder: @mutable BuildPlan) {
        builder.product = BuildProduct::Package { name: "from_generated_files" };
    }"#,
    )
    .unwrap();
    let project =
        begin_local_project_preparation(&entry, &[TargetProfile::LinuxX64], true).unwrap();
    let acquired = project.acquire_root().unwrap();
    let run = |path: &std::path::Path, sources| {
        let prepared = prepare(
            PlanningRequest {
                root: path,
                sources,
                execution: BuildExecutionInputs::default(),
                artifact_only: false,
            },
            Default::default(),
            Default::default(),
        )
        .unwrap();
        interpret(prepared, &mut ()).unwrap()
    };
    let returned = run(&entry, PlanningSources::Captured(acquired.source_capture()));
    // Replacing every generated file on the host cannot change that return.
    std::fs::rename(root.join("generated"), root.join("withdrawn")).unwrap();
    std::fs::create_dir(root.join("generated")).unwrap();
    std::fs::write(root.join("generated/configure.omg"), "machine build() {}").unwrap();
    let successor = returned.finish_successor(Vec::new()).unwrap();
    let next_entry = successor.path().to_owned();
    let result = run(&next_entry, PlanningSources::Successor(successor));
    let Some(BuildDeclaration::Package(package)) = result.returned().declaration() else {
        panic!("expected the package declared by the captured helper");
    };
    assert_eq!(package.name.as_str(), "from_generated_files");
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
    assert_eq!(checked.closed_module().identity(), checked.identity());
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
fn an_unbound_conditional_import_refuses_before_the_build_can_write() {
    configure_library();
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-conditional-import", false,
    ).unwrap();
    let root = temporary.path();
    let entry = root.join("build.omg");
    std::fs::write(root.join("main.omg"), "machine main() {}").unwrap();
    let build = r#"use main.omg as app;
use {scores}/value.omg as scores;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "fixed_wait" };
    let path: BuildPath = BuildOutput::resolve(@builder.output, "ran.txt");
    let writer: i32 = BuildOutput::create(@mutable builder.output, path, 0);
    _ = BuildOutput::write(@mutable builder.output, writer, "ran");
    _ = BuildOutput::close(@mutable builder.output, writer);
    builder.tests.disabled = false;
    transition builder.tests.disabled {
        true -> suspend later(builder),
        _ -> done(builder)
    }
    machine later(builder: @mutable BuildPlan) suspends {
        let request: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "lib" }));
        suspend Build::prepare_dependency(builder, request);
        let value: i32 = |scores|::value();
    }
    machine done(builder: @mutable BuildPlan) {
        Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    }
}
"#;
    std::fs::write(&entry, build).unwrap();
    let profile = TargetProfile::host_if_supported().unwrap();
    let project = begin_local_project_preparation(&entry, &[profile], true).unwrap();
    let acquired = project.acquire_root().unwrap();
    let rows = prepare(
        PlanningRequest {
            root: &acquired.entry_path(),
            sources: PlanningSources::Captured(acquired.source_capture()),
            execution: BuildExecutionInputs::default(),
            artifact_only: false,
        },
        Default::default(),
        Default::default(),
    ).err().expect("a Build with an unbound import must not reach interpretation")
    .into_diagnostics();
    assert!(
        rows.iter().any(|row| {
            row.message
                .contains("Build import `{scores}/value.omg as scores` has no supplied source")
                && row
                    .source_span
                    .is_some_and(|span| Some(span.span.start) == build.find("{scores}"))
        }),
        "{rows:?}"
    );
}

#[test]
fn an_artifact_only_build_requests_its_scoped_filesystem_once() {
    use build_execution::RestrictedBuildGrants;
    use package_compilation::build_execution::{RestrictedBuildOperation, RestrictedBuildRequest};
    use std::sync::{Arc, Mutex};

    struct Consent(Arc<Mutex<Vec<RestrictedBuildRequest>>>);
    impl RestrictedBuildGrants for Consent {
        fn admits(
            &mut self,
            _: &package_compilation::build_execution::RestrictedBuildSite,
            request: &RestrictedBuildRequest,
        ) -> Result<bool, Vec<diagnostics::Diagnostic>> {
            self.0.lock().unwrap().push(request.clone());
            Ok(true)
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
        r#"machine build(builder: @mutable BuildPlan) {
        builder.product = BuildProduct::Application { name: "artifacts" };
        Build::artifact_only(builder);
        _ = BuildSource::resolve(@builder.source, "main.omg");
    }"#,
    )
    .unwrap();
    let profile = TargetProfile::host_if_supported().unwrap();
    let project = begin_local_project_preparation(&entry, &[profile], true).unwrap();
    let acquired = project.acquire_root().unwrap();
    let entry = acquired.entry_path();
    let staging = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-reviewed-staging",
        false,
    )
    .unwrap();
    let sponsor = filesystem_authority::FilesystemSponsor::new(staging.path()).unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let _prepared = prepare(
        PlanningRequest {
            root: &entry,
            sources: PlanningSources::Captured(acquired.source_capture()),
            execution: BuildExecutionInputs {
                profile: Some(profile),
                directory: Some(staging.path().join("output")),
                filesystem_sponsor: Some(sponsor),
                restricted_grants: Some(Box::new(Consent(Arc::clone(&requests)))),
                ..Default::default()
            },
            artifact_only: true,
        },
        Default::default(),
        Default::default(),
    ).unwrap();
    let requests = requests.lock().unwrap();
    let [request] = requests.as_slice() else {
        panic!("one activation asked for different filesystem consent: {requests:?}");
    };
    assert_eq!(
        request.operation(),
        &RestrictedBuildOperation::ScopedFilesystemExecution
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
        r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "library" }; }"#,
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

machine build(builder: @mutable BuildPlan) {
    let valid: bool = |kit|::visible();
    transition valid { true -> named(builder), false -> wrong(builder), }
    machine named(builder: @mutable BuildPlan) {
        builder.product = BuildProduct::Application { name: "consumer" };
        Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    }
    machine wrong(builder: @mutable BuildPlan) { builder.product = BuildProduct::Application { name: "wrong" }; }
}
"#;
    std::fs::write(&entry, source).unwrap();
    let libraries = [BuildLibraryRequest {
        max_reach: Default::default(),
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
            Default::default(),
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
    // Diagnostics name the captured file by its canonical path.
    let canonical_entry = entry.canonicalize().unwrap();
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
                    .is_some_and(|location| location.path == canonical_entry && location.line == 5)
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
                max_reach: Default::default(),
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

#[test]
fn sandbox_standard_output_requires_a_bounded_sink_and_preserves_exact_bytes() {
    configure_library();
    let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-standard-output",
        false,
    )
    .unwrap();
    let entry = directory.path().join("build.omg");
    std::fs::write(
        &entry,
        r#"use main.omg as product;
use {omega}/language/core/sandbox/streams.omg as sandbox;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "with_output" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |product|::main);
    |sandbox|::SandboxOutput::write("x");
    let tail: [u8; 1] = [255u8];
    |sandbox|::SandboxOutput::write(@tail[..]);
}"#,
    )
    .unwrap();
    std::fs::write(
        directory.path().join("main.omg"),
        r#"use {omega}/language/core/sandbox/streams.omg as sandbox;
use {omega}/language/core/process_authority.omg as streams;
public machine main() reaches |streams|::StandardOutput {
    |sandbox|::SandboxOutput::write("outside the Build");
}"#,
    )
    .unwrap();
    let project =
        begin_local_project_preparation(&entry, &[TargetProfile::LinuxX64], true).unwrap();
    let acquired = project.acquire_root().unwrap();
    let run = |limit| {
        let prepared = prepare(
            PlanningRequest {
                root: &entry,
                sources: PlanningSources::Captured(acquired.source_capture()),
                execution: BuildExecutionInputs {
                    diagnostic_output_bytes: limit,
                    ..Default::default()
                },
                artifact_only: false,
            },
            Default::default(),
            Default::default(),
        )?;
        interpret(prepared, &mut ())
    };
    let refused = run(None)
        .err()
        .expect("no sink supplied")
        .into_diagnostics();
    assert!(
        refused.iter().any(|d| d.message.contains("before entry")
            && d.message.contains("StandardOutput diagnostic sink")),
        "{refused:?}"
    );
    let refused = run(Some(1))
        .err()
        .expect("second byte exceeds the activation limit")
        .into_diagnostics();
    assert!(
        refused
            .iter()
            .any(|d| d.message.contains("diagnostic sink byte limit exceeded")),
        "{refused:?}"
    );
    let mut stage = run(Some(2)).unwrap();
    let declaration = acquired.declare_build(stage.returned()).unwrap();
    let subject = declaration.identity();
    let ExecutedSourceResolution::Complete(graph) = declaration.resolve().unwrap() else {
        panic!("stock sandbox operations require no package registration")
    };
    let inputs = graph
        .join_source_result(subject, &mut stage.source_result().unwrap())
        .unwrap();
    let product = stage.open_product(Some(&inputs)).unwrap();
    assert_eq!(
        product
            .evaluated()
            .configuration
            .observation_summary
            .as_ref()
            .unwrap()
            .build_log(),
        b"x\xff"
    );
    let planned = product.product_stage(&[TargetProfile::LinuxX64]).unwrap();
    let program = planned
        .program(TargetProfile::LinuxX64, Vec::new())
        .unwrap();
    let rejected = compiler::compile(
        compiler::CompileRequest::Native {
            program,
            entry_memory: &[],
        },
        Default::default(),
    )
    .err()
    .expect("the Build's stream backing cannot supply its native Product")
    .into_diagnostics();
    assert!(
        rejected
            .iter()
            .any(|d| d.message.contains("SandboxOutput") && d.message.contains("provider")),
        "{rejected:?}"
    );
}

#[test]
fn a_product_reads_source_only_from_the_staging_directory_its_invoker_selected() {
    use package_compilation::build_execution::BuildSnapshotRequest;
    use platform_custody::temporary_directory::TemporaryDirectory;

    configure_library();
    let root = TemporaryDirectory::create("generated-from-staging", false).unwrap();
    let profile = TargetProfile::LinuxX64;
    let entry = root.path().join("build.omg");
    std::fs::write(&entry, r#"
        use main.omg as app;
        machine build(plan: @mutable BuildPlan) {
            let file: BuildPath = BuildOutput::resolve(@plan.output, "generated.omg");
            let writer: i32 = BuildOutput::create(@mutable plan.output, file, 0);
            _ = BuildOutput::write(@mutable plan.output, writer, "public machine answer() -> u32 { 17u32 }");
            _ = BuildOutput::close(@mutable plan.output, writer);
            plan.product = BuildProduct::Application { name: "from_staging" };
            Build::bind(plan, linux_x86_64::ProgramEntry, |app|::main);
        }
    "#).unwrap();
    std::fs::write(
        root.path().join("main.omg"),
        r#"
        use generated.omg;
        public machine main() crashes Trap {
            transition |generated|::answer() == 17u32 {
                true -> done(),
                false -> wrong()
            }
            machine done() {}
            machine wrong() crashes Trap { crash Trap; }
        }
    "#,
    )
    .unwrap();
    let project = begin_local_project_preparation(&entry, &[profile], true).unwrap();
    let acquired = project.acquire_root().unwrap();
    for selected in [false, true] {
        let mut snapshot = BuildSnapshotRequest::for_activation(false);
        if selected {
            snapshot = snapshot.with_product_source_directory(Vec::new()).unwrap();
        }
        let prepared = prepare(
            PlanningRequest {
                root: &entry,
                sources: PlanningSources::Captured(acquired.source_capture()),
                execution: BuildExecutionInputs {
                    snapshot: Some(snapshot),
                    ..Default::default()
                },
                artifact_only: false,
            },
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut stage = interpret(prepared, &mut ()).unwrap();
        let declaration = acquired.declare_build(stage.returned()).unwrap();
        let subject = declaration.identity();
        let ExecutedSourceResolution::Complete(graph) = declaration.resolve().unwrap() else {
            panic!("the application registers no dependencies");
        };
        let inputs = graph
            .join_source_result(subject, &mut stage.source_result().unwrap())
            .unwrap();
        let result = stage.open_product(Some(&inputs));
        if selected {
            let product = result.unwrap();
            let observed = product
                .evaluated()
                .configuration
                .observation_summary
                .as_ref()
                .unwrap();
            let tree = observed
                .product_source_tree()
                .expect("the caller selected private staging");
            assert!(tree.sealed_entry(b"generated.omg").is_some());
            let stage = product.product_stage(&[profile]).unwrap();
            compiler::compile(
                compiler::CompileRequest::Psi(stage.source()),
                Default::default(),
            )
            .unwrap();
        } else {
            let diagnostics = result
                .err()
                .expect("an ordinary scratch file must stay private")
                .into_diagnostics();
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.message.contains("generated.omg")),
                "{diagnostics:?}"
            );
        }
    }
}
